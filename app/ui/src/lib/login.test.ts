import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { classifyLoginError, detectOs, isAllowedExternalUrl } from './auth';
import { dictionaries } from './i18n';
import { createMockApi, MOCK_LOGIN_ERRORS, mockOptionsFromSearch, type MockOptions } from './mockApi';
import { accountNeedsLogin, actions, app, auth, init, keystoreMissing, launchInfo, resetStores } from './stores.svelte';

const KINDS = ['no_xbox_profile', 'country', 'adult_verification', 'child_account', 'not_owned', 'no_profile', 'offline', 'generic'];

async function boot(opts: MockOptions = {}) {
  const api = createMockApi({ timeScale: 0.01, ...opts });
  await init(api);
  return api;
}
const tick = () => vi.advanceTimersByTimeAsync(5000);

describe('login error mapping', () => {
  it('maps every stable auth code to its kind and an official help link', () => {
    const cases: [string, string][] = [
      ['auth.xbox.no_profile', 'no_xbox_profile'], ['auth.xbox.country', 'country'], ['auth.xbox.adult_verification', 'adult_verification'],
      ['auth.xbox.child_account', 'child_account'], ['auth.not_owned', 'not_owned'], ['auth.no_profile', 'no_profile'],
      ['net.request', 'offline'], ['auth.timeout', 'offline'], ['auth.login_failed', 'generic'], ['something.new', 'generic'],
    ];
    for (const [code, kind] of cases) expect(classifyLoginError({ code, message: '', needsRelogin: false }).kind, code).toBe(kind);
    expect(classifyLoginError(new Error('boom')).kind).toBe('generic');
  });
  it('every help link passes the external allow-list; offline and generic have none', () => {
    for (const code of Object.values(MOCK_LOGIN_ERRORS)) {
      const i = classifyLoginError({ code });
      if (i.kind === 'offline' || i.kind === 'generic') expect(i.help).toBeNull();
      else expect(isAllowedExternalUrl(i.help!.url), code).toBe(true);
    }
  });
  it('allow-list: https on microsoft/xbox/minecraft only', () => {
    for (const ok of ['https://www.xbox.com/regions', 'https://account.microsoft.com/family', 'https://minecraft.net/']) expect(isAllowedExternalUrl(ok), ok).toBe(true);
    for (const bad of ['http://www.xbox.com/', 'https://evilxbox.com/', 'https://xbox.com.evil.io/', 'https://u:p@xbox.com/', 'https://xbox.com:8443/', 'javascript:alert(1)', 'file:///etc/passwd', 'nope'])
      expect(isAllowedExternalUrl(bad), bad).toBe(false);
  });
  it('detects the OS for the keystore hint', () => {
    expect(detectOs('Mozilla/5.0 (X11; Linux x86_64)')).toBe('linux');
    expect(detectOs('Mozilla/5.0 (Windows NT 10.0; Win64)')).toBe('win');
    expect(detectOs('Mozilla/5.0 (Macintosh; Intel Mac OS X)')).toBe('mac');
  });
  it('has a localized message for every kind in de and en', () => {
    for (const k of KINDS) for (const l of ['de', 'en'] as const) expect(dictionaries[l][`login.err.${k}`], `${l} ${k}`).toBeTruthy();
    expect(Object.keys(MOCK_LOGIN_ERRORS).sort()).toEqual([...KINDS].sort());
  });
  it('parses the dev query params', () => {
    expect(mockOptionsFromSearch('?mockLoginError=country&mockNoKeystore&mockRelogin')).toMatchObject({ login: { error: 'country' }, noKeystore: true, needsRelogin: true });
    expect(mockOptionsFromSearch('?mockLogin=cancel').login).toBe('cancel');
    expect(mockOptionsFromSearch('').login).toBe('success');
  });
});

describe('sign in flow', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());

  it('success: pending while open, then the account is added and selected', async () => {
    await boot();
    const p = actions.signInMicrosoft();
    expect(auth.signingIn).toBe(true);
    await tick();
    const a = await p;
    expect(auth.signingIn).toBe(false);
    expect(a && a.name).toBe('CreeperKing');
    expect(app.accounts.map((x) => x.name)).toContain('CreeperKing');
    expect(app.config?.selectedAccount).toBe(a && a.uuid);
    expect(auth.error).toBeNull();
  });
  it('cancel resolves null silently: no error, no account', async () => {
    await boot({ login: 'cancel' });
    const p = actions.signInMicrosoft();
    await tick();
    expect(await p).toBeNull();
    expect(auth.error).toBeNull();
    expect(auth.signingIn).toBe(false);
    expect(app.accounts).toHaveLength(2);
  });
  for (const variant of Object.keys(MOCK_LOGIN_ERRORS)) {
    it(`error variant ${variant} sets a classified error`, async () => {
      await boot({ login: { error: variant } });
      const p = actions.signInMicrosoft();
      await tick();
      expect(await p).toBeUndefined();
      expect(auth.error?.kind).toBe(variant);
      expect(auth.signingIn).toBe(false);
      expect(app.accounts).toHaveLength(2);
    });
  }
  it('ignores a second click while the window is open', async () => {
    const api = await boot();
    const spy = vi.spyOn(api, 'accountSignInMicrosoft');
    void actions.signInMicrosoft();
    expect(await actions.signInMicrosoft()).toBeUndefined();
    await tick();
    expect(spy).toHaveBeenCalledTimes(1);
  });
});

describe('account states', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());

  it('flags needsRelogin / expired accounts, not offline ones', async () => {
    await boot({ needsRelogin: true });
    expect(app.accounts.map(accountNeedsLogin)).toEqual([true, false]);
  });
  it('keystore notice shows only when some account is memory-only', async () => {
    await boot();
    expect(keystoreMissing()).toBe(false);
    resetStores();
    await boot({ noKeystore: true });
    expect(keystoreMissing()).toBe(true);
    auth.noticeDismissed = true; // dismissal hides the notice but the rail indicator condition stays
    expect(keystoreMissing()).toBe(true);
  });
  it('a locked account is reported as such', async () => {
    await boot({ locked: true });
    expect(app.accounts[0].credentialsLocked).toBe(true);
  });
  it('accounts:changed refreshes the store', async () => {
    const api = await boot({ needsRelogin: true });
    expect(app.accounts[0].needsRelogin).toBe(true);
    const p = api.accountSignInMicrosoft();
    await tick();
    await p;
    await tick();
    expect(app.accounts[0].needsRelogin).toBe(false);
  });
  it('account_refresh updates the account; a dead session just flips to relogin', async () => {
    await boot();
    const uuid = app.accounts[0].uuid;
    const p = actions.refreshAccount(uuid);
    await tick();
    expect((await p)?.uuid).toBe(uuid);
    expect(auth.refreshing[uuid]).toBeUndefined();
    resetStores();
    await boot({ needsRelogin: true });
    const q = actions.refreshAccount(app.accounts[0].uuid);
    await tick();
    expect(await q).toBeUndefined();
    expect(app.error).toBeNull(); // needsRelogin is not reported as a failure
  });
});

describe('launch needing a new login', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());

  it('opens the relogin request, then retries the launch after a successful sign-in', async () => {
    await boot({ needsRelogin: true });
    const id = app.profiles[0].id;
    const play = actions.play(id);
    await tick();
    await play;
    expect(auth.relogin).toEqual({ profileId: id });
    expect(app.error).toBeNull();
    expect(launchInfo(id).state).toBe('stopped');

    const retry = actions.reloginAndRetry();
    expect(auth.signingIn).toBe(true);
    await tick();
    await retry;
    expect(auth.relogin).toBeNull();
    await vi.advanceTimersByTimeAsync(7000);
    expect(launchInfo(id).state).toBe('running');
  });
  it('keeps the quick-join target for the retry', async () => {
    await boot({ needsRelogin: true });
    const id = app.profiles[0].id;
    const p = actions.play(id, 'mc.example.net');
    await tick();
    await p;
    expect(auth.relogin).toEqual({ profileId: id, quickJoin: 'mc.example.net' });
  });
  it('cancel or error keeps the dialog open and does not launch', async () => {
    await boot({ needsRelogin: true, login: 'cancel' });
    const id = app.profiles[0].id;
    const p = actions.play(id);
    await tick();
    await p;
    const r = actions.reloginAndRetry();
    await tick();
    await r;
    expect(auth.relogin).not.toBeNull();
    expect(auth.error).toBeNull();
    expect(launchInfo(id).state).toBe('stopped');

    resetStores();
    await boot({ needsRelogin: true, login: { error: 'not_owned' } });
    const p2 = actions.play(app.profiles[0].id);
    await tick();
    await p2;
    const r2 = actions.reloginAndRetry();
    await tick();
    await r2;
    expect(auth.relogin).not.toBeNull();
    expect(auth.error?.kind).toBe('not_owned');
  });
  it('other launch errors still surface as a normal error', async () => {
    const api = await boot();
    vi.spyOn(api, 'gameLaunch').mockRejectedValue({ code: 'launch_failed', message: 'x', needsRelogin: false });
    await actions.play(app.profiles[0].id);
    expect(auth.relogin).toBeNull();
    expect(app.error?.code).toBe('launch_failed');
    expect(launchInfo(app.profiles[0].id).state).toBe('crashed');
  });
});

describe('external links', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());
  it('only allowed URLs reach the api', async () => {
    const api = await boot();
    const spy = vi.spyOn(api, 'openExternal');
    await actions.openExternal('https://evil.example/');
    expect(spy).not.toHaveBeenCalled();
    await actions.openExternal('https://account.xbox.com/');
    expect(spy).toHaveBeenCalledWith({ url: 'https://account.xbox.com/' });
  });
});
