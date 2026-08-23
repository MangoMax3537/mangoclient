'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const P = require('./paths');

/**
 * MangoConfig - MangoClient's own in-game HUD and settings layer.
 *
 * The mod is built from `mod/` in this repository and ships inside the app, so
 * there is nothing to download and nothing to keep in step with a third party.
 * On every launch the jar is either put into the instance's mods folder or
 * taken back out of it, depending on whether the profile wants it.
 *
 * One jar per Minecraft version: the mod is compiled once per game version it
 * supports (`MangoConfig-<mod>+mc<game>.jar`), and the launch picks the build
 * that matches the profile. A version with no build simply starts without.
 *
 * It is deliberately absent from the launcher's mod list: the player did not
 * install it, cannot update it separately, and should not have to scroll past
 * it. The switch in the statusbar is the whole interface it needs.
 */

const NAME = 'MangoConfig';
/** Bundled with the app; `src/**` is what electron-builder packs. */
const ASSETS = path.join(__dirname, 'assets');
/** Any build of ours, so an older one can be recognised and cleared out. */
const OWN_JAR_RE = /^MangoConfig-.*\.jar(\.disabled)?$/i;
/** The per-version builds: MangoConfig-1.7.0+mc1.21.4.jar and friends. */
const VERSIONED_RE = /^MangoConfig-.+\+mc(.+)\.jar$/i;

/** Game version -> absolute path of the matching bundled jar. */
const JARS = (() => {
	const map = {};
	try {
		for (const name of fs.readdirSync(ASSETS)) {
			const m = VERSIONED_RE.exec(name);
			if (m) map[m[1]] = path.join(ASSETS, name);
		}
	} catch { /* a build without assets ships no mod */ }
	return map;
})();

/** The mod needs a Fabric-style loader; Quilt loads Fabric mods natively. */
const GAME_VERSIONS = Object.keys(JARS).sort((a, b) =>
	a.localeCompare(b, undefined, { numeric: true }));
const LOADERS = ['fabric', 'quilt'];

function enabledFor(profile, config) {
	if (profile?.mangoConfig === false) return false;
	return config?.mangoConfig !== false;
}

function supports(profile) {
	return LOADERS.includes(profile.loader) && JARS[profile.mcVersion] != null;
}

/** Whether a build exists for the version at all, whatever the loader. */
function hasBuildFor(mcVersion) {
	return JARS[mcVersion] != null;
}

function jarIn(profileId, jarName) {
	return path.join(P.instanceDir(profileId), 'mods', jarName);
}

/** Same jar already there? Compare size; the file is ours and never edited. */
async function upToDate(target, source) {
	try {
		const [a, b] = await Promise.all([fsp.stat(target), fsp.stat(source)]);
		return a.size === b.size;
	} catch {
		return false;
	}
}

/**
 * Put MangoConfig into the instance, or take it out again.
 *
 * Never throws: a HUD is a worse reason to refuse a launch than to start
 * without one, so every failure is reported and stepped over.
 */
async function ensure({ profile, config, onLog = () => {} }) {
	const wanted = enabledFor(profile, config);

	if (!wanted) {
		const removed = await removeOurJars(profile.id);
		if (removed) onLog(`${NAME} removed from this profile`);
		return { state: removed ? 'removed' : 'off' };
	}

	if (!supports(profile)) {
		// Leave nothing behind if the profile was switched to a version we have
		// no build for, or the game would refuse to start.
		await removeOurJars(profile.id);
		return { state: 'unsupported', reason: `${profile.loader} ${profile.mcVersion}` };
	}

	const source = JARS[profile.mcVersion];
	const jarName = path.basename(source);
	const target = jarIn(profile.id, jarName);

	try {
		if (await upToDate(target, source)) {
			await removeOurJars(profile.id, { except: jarName });
			return { state: 'current' };
		}
		await fsp.mkdir(path.dirname(target), { recursive: true });
		// An older build has to go first: two jars, one mod id, no launch.
		await removeOurJars(profile.id, { except: jarName });
		await fsp.copyFile(source, target);
		onLog(`${NAME} added to this profile`);
		return { state: 'installed' };
	} catch (err) {
		return { state: 'failed', error: err.message };
	}
}

/** True for any build of ours, so the mod list can skip it. */
function isOwnJar(filename) {
	return OWN_JAR_RE.test(path.basename(filename || ''));
}

/**
 * Clear out builds that are not the current one.
 *
 * Two jars declaring the same mod id stop Fabric from starting, so an upgrade
 * that changes the file name has to take the old file with it.
 */
async function removeOurJars(profileId, { except = null } = {}) {
	const dir = path.join(P.instanceDir(profileId), 'mods');
	let names;
	try {
		names = await fsp.readdir(dir);
	} catch {
		return 0;
	}
	let removed = 0;
	for (const name of names) {
		if (!isOwnJar(name) || name === except) continue;
		try {
			await fsp.unlink(path.join(dir, name));
			removed++;
		} catch { /* in use; the next launch tries again */ }
	}
	return removed;
}

function present(profileId) {
	const dir = path.join(P.instanceDir(profileId), 'mods');
	try {
		return fs.readdirSync(dir).some((name) => VERSIONED_RE.test(name));
	} catch {
		return false;
	}
}

module.exports = {
	ensure,
	removeOurJars,
	enabledFor,
	supports,
	hasBuildFor,
	isOwnJar,
	present,
	NAME,
	GAME_VERSIONS,
	LOADERS,
};
