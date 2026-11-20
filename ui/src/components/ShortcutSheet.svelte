<script lang="ts">
  // The keyboard shortcut sheet: every global shortcut, grouped, with platform-aware key caps (Ctrl on Windows/Linux, the Command key on macOS).
  import Modal from './Modal.svelte';
  import { SHORTCUT_GROUPS, capText } from '../lib/keybinds';
  import { lang, platform, t } from '../lib/stores.svelte';

  let { onclose }: { onclose: () => void } = $props();
</script>

<Modal title={t('keys.title')} size="lg" {onclose}>
  <div class="sheet">
    {#each SHORTCUT_GROUPS as g (g.id)}
      <section aria-labelledby="kg-{g.id}">
        <h3 id="kg-{g.id}" class="eyebrow">{t(g.title)}</h3>
        <dl>
          {#each g.items as it (it.id)}
            <div class="row">
              <dt>{t(it.label)}</dt>
              <dd>
                {#each it.keys(platform.mac) as combo, i (i)}
                  {#if i > 0}<span class="or">{t('keys.or')}</span>{/if}
                  <span class="combo">{#each combo as k, j (j)}<kbd>{capText(k, lang())}</kbd>{/each}</span>
                {/each}
              </dd>
            </div>
          {/each}
        </dl>
      </section>
    {/each}
    <p class="hint note">{t('keys.note')}</p>
  </div>
</Modal>

<style>
  .sheet { display: grid; gap: 18px; }
  section { display: grid; gap: 6px; }
  h3 { margin: 0; font-family: var(--font-ui); }
  dl { margin: 0; display: grid; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 7px 0; }
  .row + .row { border-top: 1px solid var(--line-soft); }
  dt { color: var(--paper); }
  dd { margin: 0; display: flex; align-items: center; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
  .combo { display: inline-flex; gap: 4px; }
  .or { color: var(--paper-mute); font-size: var(--fs-xs); }
  kbd {
    display: inline-grid; place-items: center; min-width: 26px; height: 24px; padding: 0 7px; font: 500 12px var(--font-mono); color: var(--paper);
    background: var(--ink-3); border: 1px solid var(--line); box-shadow: inset 0 -2px 0 var(--line); 
 }
  .note { margin: 0; }
</style>
