'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const { spawn } = require('child_process');
const { EventEmitter } = require('events');
const P = require('./paths');
const { installVersion, resolveVersionJson, rulesAllow } = require('./installer');
const { ensureLoader } = require('./loaders');
const { resolveJava } = require('./java');
const { ensureFreshAccount } = require('./auth');

const CP_SEP = process.platform === 'win32' ? ';' : ':';

/**
 * JVM flag sets. "balanced" is the G1 tuning popularised by Aikar, which keeps
 * GC pauses short for typical 4-8 GB heaps.
 */
const PRESETS = {
  potato: [
    '-XX:+UseSerialGC', '-XX:TieredStopAtLevel=1', '-Dsun.rmi.dgc.server.gcInterval=2147483646',
  ],
  balanced: [
    '-XX:+UseG1GC', '-XX:+ParallelRefProcEnabled', '-XX:MaxGCPauseMillis=200',
    '-XX:+UnlockExperimentalVMOptions', '-XX:+DisableExplicitGC', '-XX:+AlwaysPreTouch',
    '-XX:G1NewSizePercent=30', '-XX:G1MaxNewSizePercent=40', '-XX:G1HeapRegionSize=8M',
    '-XX:G1ReservePercent=20', '-XX:G1HeapWastePercent=5', '-XX:G1MixedGCCountTarget=4',
    '-XX:InitiatingHeapOccupancyPercent=15', '-XX:G1MixedGCLiveThresholdPercent=90',
    '-XX:G1RSetUpdatingPauseTimePercent=5', '-XX:SurvivorRatio=32',
    '-XX:+PerfDisableSharedMem', '-XX:MaxTenuringThreshold=1',
  ],
  quality: [
    '-XX:+UseZGC', '-XX:+ZGenerational', '-XX:+AlwaysPreTouch', '-XX:+DisableExplicitGC',
  ],
};

function substitute(str, vars) {
  return String(str).replace(/\$\{([^}]+)\}/g, (whole, key) =>
    Object.prototype.hasOwnProperty.call(vars, key) ? vars[key] : whole);
}

/** Flatten Mojang's modern argument arrays, honouring rules and feature flags. */
function expandArgs(args, vars, features) {
  const out = [];
  for (const arg of args || []) {
    if (typeof arg === 'string') {
      out.push(substitute(arg, vars));
      continue;
    }
    if (!rulesAllow(arg.rules, features)) continue;
    const value = Array.isArray(arg.value) ? arg.value : [arg.value];
    for (const v of value) out.push(substitute(v, vars));
  }
  return out;
}

/** Mirror `options.txt` settings we care about without clobbering user choices. */
async function seedInstanceOptions(gameDir, { language }) {
  const optionsFile = path.join(gameDir, 'options.txt');
  if (fs.existsSync(optionsFile)) return;
  const lang = language === 'de' ? 'de_de' : 'en_us';
  await fsp.mkdir(gameDir, { recursive: true });
  await fsp.writeFile(optionsFile, [
    `lang:${lang}`,
    'guiScale:0',
    'maxFps:260',
    'enableVsync:false',
    'renderDistance:12',
    'soundCategory_master:0.5',
    'autoJump:false',
    'toggleCrouch:false',
    'pauseOnLostFocus:false',
  ].join('\n') + '\n');
}

class GameInstance extends EventEmitter {
  constructor(profileId, child, versionId) {
    super();
    this.profileId = profileId;
    this.child = child;
    this.versionId = versionId;
    this.startedAt = Date.now();
    this.pid = child.pid;
  }
  kill() {
    try { this.child.kill('SIGTERM'); } catch { /* already gone */ }
  }
  /**
   * Persist the session's play time. Quitting the launcher kills the game and
   * exits before the child's 'close' event can fire, so the shutdown path calls
   * this directly, hence the guard against counting a session twice.
   */
  flushPlayTime() {
    if (this.playTimeFlushed) return;
    this.playTimeFlushed = true;
    this.onFlushPlayTime?.(Date.now() - this.startedAt);
  }
}

/** Turn one <log4j:Event> block into readable console lines. */
function emitLogEvent(xml, emit) {
  const attr = (name) => (xml.match(new RegExp(`\\b${name}="([^"]*)"`)) || [])[1] || '';
  const cdata = (tag) =>
    (xml.match(new RegExp(`<log4j:${tag}><!\\[CDATA\\[([\\s\\S]*?)\\]\\]></log4j:${tag}>`)) || [])[1] || '';

  const level = attr('level');
  const stamp = Number(attr('timestamp'));
  const time = new Date(Number.isFinite(stamp) && stamp ? stamp : Date.now())
    .toLocaleTimeString('en-GB', { hour12: false });

  const throwable = cdata('Throwable');
  const body = throwable ? `${cdata('Message')}\n${throwable.trimEnd()}` : cdata('Message');
  const kind = level === 'ERROR' || level === 'FATAL' ? 'error' : 'info';

  const [first, ...rest] = body.split('\n');
  emit(`[${time}] [${attr('thread')}/${level}]: ${first}`, kind);
  // Stack-trace lines keep their own indentation instead of a repeated prefix.
  for (const line of rest) emit(line, kind);
}

/**
 * Minecraft's log4j configuration writes XML events to stdout rather than plain
 * lines, so the raw stream is unreadable in a console view. Reassemble each
 * event across chunk boundaries and format it; anything that is not an event
 * (JVM warnings, Fabric's early output) passes through untouched.
 */
function createLogParser(emit) {
  const OPEN = '<log4j:Event';
  const CLOSE = '</log4j:Event>';
  let buffer = '';

  const flushPlain = (text) => {
    for (const line of text.split('\n')) {
      const trimmed = line.replace(/\s+$/, '');
      if (trimmed) emit(trimmed);
    }
  };

  return (chunk) => {
    buffer += chunk;
    for (;;) {
      const start = buffer.indexOf(OPEN);
      if (start === -1) {
        // No event in flight: emit complete lines, hold back a partial one.
        const nl = buffer.lastIndexOf('\n');
        if (nl === -1) return;
        flushPlain(buffer.slice(0, nl));
        buffer = buffer.slice(nl + 1);
        return;
      }
      if (start > 0) {
        flushPlain(buffer.slice(0, start));
        buffer = buffer.slice(start);
        continue;
      }
      const end = buffer.indexOf(CLOSE);
      if (end === -1) return; // event still arriving
      emitLogEvent(buffer.slice(0, end + CLOSE.length), emit);
      buffer = buffer.slice(end + CLOSE.length);
    }
  };
}

/**
 * Install (if needed) and launch a profile.
 * Emits progress/log events through the supplied callbacks; resolves with a
 * GameInstance once the JVM process is spawned.
 */
async function launch({
  profile, account, config, store,
  onProgress = () => {}, onLog = () => {}, onState = () => {}, quickJoinServer = null,
}) {
  const log = (line) => onLog(line);

  onState('preparing');
  log(`Preparing ${profile.name} (Minecraft ${profile.mcVersion}, ${profile.loader})`);

  // 1. A JVM is needed before Forge-style installers can run, so resolve Java
  //    against the vanilla metadata first.
  const baseVersionJson = await resolveVersionJson(profile.mcVersion);
  onState('java');
  const java = await resolveJava(baseVersionJson, {
    override: config.javaPath || null,
    onProgress,
    onLog: log,
  });
  log(`Java ${java.major} (${java.source}): ${java.path}`);

  // 2. Loader
  onState('loader');
  const { versionId, loaderVersion } = await ensureLoader({
    loader: profile.loader,
    mcVersion: profile.mcVersion,
    loaderVersion: profile.loaderVersion,
    javaPath: java.path,
    onLog: log,
  });
  if (loaderVersion && loaderVersion !== profile.loaderVersion) {
    store?.updateProfile(profile.id, { loaderVersion });
  }

  // 3. Files
  onState('downloading');
  const install = await installVersion(versionId, {
    onProgress,
    onLog: log,
    concurrency: config.concurrentDownloads || 12,
  });
  const version = install.version;

  // Loaders bump the Java requirement, so re-check now that we know the real version.
  let javaPath = java.path;
  if ((version.javaVersion?.majorVersion || 0) > java.major) {
    onState('java');
    const upgraded = await resolveJava(version, { override: config.javaPath || null, onProgress, onLog: log });
    javaPath = upgraded.path;
    log(`Switched to Java ${upgraded.major} for ${version.id}`);
  }

  // 4. Account
  onState('account');
  const fresh = await ensureFreshAccount(account);
  if (fresh !== account) store?.upsertAccount(fresh);

  // 5. Arguments
  const gameDir = P.instanceDir(profile.id);
  await fsp.mkdir(path.join(gameDir, 'mods'), { recursive: true });
  await seedInstanceOptions(gameDir, { language: config.language });

  const ram = profile.ram || config.ram || 4096;
  const classpath = [...install.classpath, install.clientJar];

  const features = {
    is_demo_user: false,
    has_custom_resolution: !config.fullscreen,
    has_quick_plays_support: Boolean(quickJoinServer),
    is_quick_play_multiplayer: Boolean(quickJoinServer),
    is_quick_play_singleplayer: false,
    is_quick_play_realms: false,
  };

  const vars = {
    auth_player_name: fresh.name,
    auth_uuid: fresh.uuid,
    auth_access_token: fresh.accessToken,
    auth_session: `token:${fresh.accessToken}:${fresh.uuid}`,
    auth_xuid: fresh.xuid || '0',
    user_type: fresh.type === 'offline' ? 'legacy' : 'msa',
    user_properties: '{}',
    clientid: '',
    version_name: version.id,
    version_type: version.type || 'release',
    game_directory: gameDir,
    assets_root: P.assets,
    game_assets: install.isLegacyAssets
      ? path.join(P.assets, 'virtual', install.assetsIndexId)
      : P.assets,
    assets_index_name: install.assetsIndexId,
    natives_directory: install.nativesDir,
    launcher_name: 'MangoClient',
    launcher_version: '1.0.0',
    classpath: classpath.join(CP_SEP),
    classpath_separator: CP_SEP,
    library_directory: P.libraries,
    resolution_width: String(config.width || 1280),
    resolution_height: String(config.height || 720),
    quickPlayMultiplayer: quickJoinServer || '',
    quickPlayPath: '',
    quickPlaySingleplayer: '',
    quickPlayRealms: '',
  };

  const jvmArgs = [];
  jvmArgs.push(`-Xmx${ram}M`, `-Xms${Math.min(ram, 1024)}M`);
  jvmArgs.push(...(PRESETS[config.performancePreset] || PRESETS.balanced));
  jvmArgs.push('-Dfile.encoding=UTF-8', '-Dstdout.encoding=UTF-8', '-Dstderr.encoding=UTF-8');
  jvmArgs.push(`-Dminecraft.launcher.brand=MangoClient`, `-Dminecraft.launcher.version=1.0.0`);
  if (process.platform === 'darwin') jvmArgs.push('-XstartOnFirstThread');
  if (install.loggingArg) jvmArgs.push(install.loggingArg);

  if (version.arguments?.jvm) {
    jvmArgs.push(...expandArgs(version.arguments.jvm, vars, features));
  } else {
    // Pre-1.13 versions have no jvm argument list at all.
    jvmArgs.push(`-Djava.library.path=${install.nativesDir}`, '-cp', vars.classpath);
  }

  for (const extra of [config.javaArgs, profile.javaArgs]) {
    if (extra && extra.trim()) jvmArgs.push(...extra.trim().split(/\s+/));
  }

  const gameArgs = version.arguments?.game
    ? expandArgs(version.arguments.game, vars, features)
    : substitute(version.minecraftArguments || '', vars).split(/\s+/).filter(Boolean);

  if (config.fullscreen) gameArgs.push('--fullscreen');
  else if (!gameArgs.includes('--width')) {
    gameArgs.push('--width', String(config.width || 1280), '--height', String(config.height || 720));
  }
  if (quickJoinServer && !gameArgs.includes('--quickPlayMultiplayer')) {
    const [host, port = '25565'] = quickJoinServer.split(':');
    gameArgs.push('--server', host, '--port', port);
  }

  const args = [...jvmArgs, version.mainClass, ...gameArgs];

  // 6. Go
  onState('launching');
  log(`Launching: ${path.basename(javaPath)} ${version.mainClass} (${ram} MB heap)`);
  const child = spawn(javaPath, args, {
    cwd: gameDir,
    env: { ...process.env, INST_NAME: profile.name, INST_ID: profile.id, INST_DIR: gameDir },
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  const instance = new GameInstance(profile.id, child, version.id);
  instance.onFlushPlayTime = (played) => {
    store?.updateProfile(profile.id, {
      lastPlayed: Date.now(),
      playTimeMs: (store.getProfile(profile.id)?.playTimeMs || 0) + played,
    });
  };

  let sawWindow = false;
  const handle = (stream, level) => {
    const feed = createLogParser((line, lineLevel) => {
      onLog(line, lineLevel || level);
      if (!sawWindow && /Setting user:|LWJGL Version|Created:.*minecraft:textures|OpenAL initialized/.test(line)) {
        sawWindow = true;
        onState('running');
        instance.emit('running');
      }
    });
    stream.on('data', (chunk) => feed(chunk.toString()));
  };
  handle(child.stdout, 'info');
  handle(child.stderr, 'error');

  child.on('error', (err) => {
    onLog(`Failed to start the game: ${err.message}`, 'error');
    onState('crashed');
    instance.emit('exit', -1);
  });

  child.on('close', (code) => {
    instance.flushPlayTime();
    onLog(`Game exited with code ${code}`, code === 0 ? 'info' : 'error');
    onState(code === 0 ? 'stopped' : 'crashed');
    instance.emit('exit', code);
  });

  // Nothing in the log confirms the window for every version, so assume success
  // if the process is still alive shortly after start.
  setTimeout(() => {
    if (!sawWindow && child.exitCode === null) {
      sawWindow = true;
      onState('running');
      instance.emit('running');
    }
  }, 8000);

  return instance;
}

module.exports = { launch, PRESETS, GameInstance };
