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
 * It is deliberately absent from the launcher's mod list: the player did not
 * install it, cannot update it separately, and should not have to scroll past
 * it. The switch in the statusbar is the whole interface it needs.
 */

const NAME = 'MangoConfig';
/** Bundled with the app; `src/**` is what electron-builder packs. */
const JAR = path.join(__dirname, 'assets', 'MangoConfig-1.0.0.jar');
const JAR_NAME = path.basename(JAR);

/**
 * The mod is compiled against one Minecraft version and needs a Fabric-style
 * loader. Quilt loads Fabric mods natively; the rest get nothing rather than a
 * jar that would stop the game from starting.
 */
const GAME_VERSIONS = ['1.21.11'];
const LOADERS = ['fabric', 'quilt'];

function enabledFor(profile, config) {
	if (profile?.mangoConfig === false) return false;
	return config?.mangoConfig !== false;
}

function supports(profile) {
	return LOADERS.includes(profile.loader) && GAME_VERSIONS.includes(profile.mcVersion);
}

function jarIn(profileId) {
	return path.join(P.instanceDir(profileId), 'mods', JAR_NAME);
}

/** Same jar already there? Compare size; the file is ours and never edited. */
async function upToDate(target) {
	try {
		const [a, b] = await Promise.all([fsp.stat(target), fsp.stat(JAR)]);
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
	const target = jarIn(profile.id);
	const wanted = enabledFor(profile, config);

	if (!wanted) {
		try {
			await fsp.unlink(target);
			onLog(`${NAME} removed from this profile`);
			return { state: 'removed' };
		} catch {
			return { state: 'off' };
		}
	}

	if (!supports(profile)) {
		// Leave nothing behind if the profile was switched to a version we
		// have no build for, or the game would refuse to start.
		await fsp.unlink(target).catch(() => {});
		return { state: 'unsupported', reason: `${profile.loader} ${profile.mcVersion}` };
	}

	try {
		if (await upToDate(target)) return { state: 'current' };
		await fsp.mkdir(path.dirname(target), { recursive: true });
		await fsp.copyFile(JAR, target);
		onLog(`${NAME} added to this profile`);
		return { state: 'installed' };
	} catch (err) {
		return { state: 'failed', error: err.message };
	}
}

/** True for the jar the launcher manages, so the mod list can skip it. */
function isOwnJar(filename) {
	return path.basename(filename || '') === JAR_NAME;
}

function present(profileId) {
	return fs.existsSync(jarIn(profileId));
}

module.exports = {
	ensure,
	enabledFor,
	supports,
	isOwnJar,
	present,
	NAME,
	JAR_NAME,
	GAME_VERSIONS,
	LOADERS,
};
