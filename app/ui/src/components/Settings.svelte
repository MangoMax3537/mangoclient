<script lang="ts">
  import { fmtRam } from '../lib/format';
  import { actions, app, t } from '../lib/stores.svelte';

  const cfg = $derived(app.config);
  const maxRam = $derived(Math.max(2048, app.totalRamMb || 8192));
  let args = $state(app.config?.javaArgs ?? '');
  let argTimer: ReturnType<typeof setTimeout> | undefined;
  function onArgs() {
    clearTimeout(argTimer);
    argTimer = setTimeout(() => actions.setConfig({ javaArgs: args }), 400);
  }
</script>

{#if cfg}
  <div class="settings">
    <h1>{t('settings.title')}</h1>

    <section aria-labelledby="s-launcher">
      <h2 id="s-launcher" class="eyebrow">{t('settings.group.general')}</h2>

      <div class="row">
        <div class="txt"><label for="lang">{t('settings.language')}</label></div>
        <select id="lang" class="field" value={cfg.language} onchange={(e) => actions.setConfig({ language: e.currentTarget.value as 'de' | 'en' })}>
          <option value="de">{t('settings.language.de')}</option>
          <option value="en">{t('settings.language.en')}</option>
        </select>
      </div>

      <div class="row">
        <div class="txt"><label for="keep">{t('settings.keepOpen')}</label><div class="hint">{t('settings.keepOpenHint')}</div></div>
        <span class="switch"><input id="keep" type="checkbox" checked={cfg.keepLauncherOpen} onchange={(e) => actions.setConfig({ keepLauncherOpen: e.currentTarget.checked })} /><span></span></span>
      </div>

      <div class="row">
        <div class="txt"><label for="tele">{t('settings.telemetry')}</label><div class="hint">{t('settings.telemetryHint')}</div></div>
        <span class="switch"><input id="tele" type="checkbox" checked={cfg.telemetry} onchange={(e) => actions.setConfig({ telemetry: e.currentTarget.checked })} /><span></span></span>
      </div>
    </section>

    <section aria-labelledby="s-game">
      <h2 id="s-game" class="eyebrow">{t('settings.group.game')}</h2>

      <div class="row col">
        <div class="txt"><label for="ram">{t('settings.ram')}</label></div>
        <div class="ram">
          <input id="ram" type="range" min="1024" max={maxRam} step="512" value={cfg.ram} aria-valuetext={fmtRam(cfg.ram)}
            oninput={(e) => actions.setConfig({ ram: Number(e.currentTarget.value) })} />
          <output class="mono"><b>{fmtRam(cfg.ram)}</b> <span class="hint">{t('settings.ramOf', { total: fmtRam(app.totalRamMb) })}</span></output>
        </div>
      </div>

      <div class="row col">
        <div class="txt"><label for="jargs">{t('settings.javaArgs')}</label><div class="hint">{t('settings.javaArgsHint')}</div></div>
        <input id="jargs" class="field mono" bind:value={args} oninput={onArgs} spellcheck="false" autocomplete="off" />
      </div>
    </section>

    <footer class="hint mono">{t('settings.version')} {app.version} &middot; {t('settings.dataDir')}: {app.rootDir}</footer>
  </div>
{/if}

<style>
  .settings { padding: 20px 28px 32px; max-width: 760px; display: grid; gap: 24px; height: 100%; overflow: auto; align-content: start; }
  h1 { font-size: 34px; }
  section { display: grid; border-top: 1px solid var(--line); }
  section h2 { padding: 14px 0 6px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 12px 0; border-bottom: 1px solid var(--line-soft); }
  .row.col { flex-direction: column; align-items: stretch; gap: 8px; }
  .txt label { font-weight: 500; }
  .ram { display: flex; align-items: center; gap: 16px; }
  .ram output { white-space: nowrap; font-size: 13px; }
  .ram b { color: var(--mango); font-weight: 500; }
  footer { word-break: break-all; }
</style>
