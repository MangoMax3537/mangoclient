'use strict';
const { app } = require('electron');
const { autoUpdater } = require('electron-updater');

/**
 * Auto-update.
 *
 * electron-updater fetches a small YAML manifest from wherever the build was
 * published (see `build.publish` in package.json), compares its version with
 * this one and downloads the installer in the background. That manifest and the
 * installer are plain static files. No server of ours is involved, and there
 * is nothing to store per user.
 *
 * The download is applied on quit, so a player who leaves the launcher open
 * for days still gets the update the next time they close it.
 */
/** Players leave a launcher open for days, so re-check while it runs. */
const RECHECK_INTERVAL_MS = 3 * 60 * 60 * 1000;

function createUpdater({ onState }) {
  let last = { state: 'idle', version: app.getVersion() };
  let timer = null;
  /**
   * Only the check at launch restarts on its own. A periodic check that did
   * the same would yank the launcher away from someone mid-session.
   */
  let applyOnDownload = false;

  const emit = (patch) => {
    last = { ...patch, current: app.getVersion(), applying: applyOnDownload };
    onState(last);
  };

  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;

  autoUpdater.on('checking-for-update', () => emit({ state: 'checking' }));
  autoUpdater.on('update-available', (info) => emit({ state: 'downloading', version: info.version, percent: 0 }));
  autoUpdater.on('update-not-available', () => emit({ state: 'current' }));
  autoUpdater.on('download-progress', (p) =>
    emit({ state: 'downloading', version: last.version, percent: Math.round(p.percent || 0) }));
  autoUpdater.on('update-downloaded', (info) => {
    emit({ state: 'ready', version: info.version });
    if (!applyOnDownload) return;
    // Give the renderer a moment to paint the closing message.
    setTimeout(() => autoUpdater.quitAndInstall(true, true), 800);
  });
  autoUpdater.on('error', (err) => {
    // Never strand the player on the update screen because GitHub was down.
    applyOnDownload = false;
    emit({ state: 'error', error: String(err?.message || err) });
  });

  const api = {
    get state() { return last; },

    /**
     * Keep checking for as long as the launcher stays open. Without this, a
     * player who never closes it would only ever see the version they started
     * with. Nothing is pushed from our side; this is still the app asking.
     */
    startPeriodicChecks(intervalMs = RECHECK_INTERVAL_MS) {
      if (timer) clearInterval(timer);
      timer = setInterval(() => {
        // Nothing to gain from re-checking mid-download or once it is staged.
        if (last.state === 'downloading' || last.state === 'ready') return;
        api.check();
      }, intervalMs);
      // Do not hold the process open just for the timer.
      timer.unref?.();
      return api;
    },

    /**
     * `apply` installs and relaunches as soon as the download finishes.
     * It is used for the check at launch, so the player lands on the new version
     * straight away instead of being asked.
     */
    async check({ apply = false } = {}) {
      applyOnDownload = apply;

      // A dev run has no packaged metadata to compare against.
      if (!app.isPackaged) {
        applyOnDownload = false;
        emit({ state: 'disabled', reason: 'dev' });
        return last;
      }
      // A tar.gz install cannot replace itself; AppImage and the Windows
      // installer can. Saying so beats failing silently.
      if (process.platform === 'linux' && !process.env.APPIMAGE) {
        applyOnDownload = false;
        emit({ state: 'disabled', reason: 'unsupported' });
        return last;
      }
      try {
        await autoUpdater.checkForUpdates();
      } catch (err) {
        applyOnDownload = false;
        emit({ state: 'error', error: String(err?.message || err) });
      }
      return last;
    },

    install() {
      if (last.state !== 'ready') return false;
      autoUpdater.quitAndInstall();
      return true;
    },
  };

  return api;
}

module.exports = { createUpdater };
