<script lang="ts">
  // Group and tags of an instance (instance settings). Validation mirrors the service (48 / 12 x 24 chars, trimmed, no control characters,
  // case-insensitive duplicates); every change is saved at once through profile_update.
  import { untrack } from 'svelte';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import TagIcon from 'phosphor-svelte/lib/TagIcon';
  import { checkGroup, checkTag, GROUP_MAX, groupErrorKey, knownGroups, knownTags, TAG_MAX, TAGS_MAX, tagErrorKey } from '../lib/portability';
  import { actions, app, errorText, t, toast } from '../lib/stores.svelte';
  import type { ProfileView } from '../lib/types';

  let { profile, onsaved }: { profile: ProfileView; onsaved?: () => void } = $props();
  let group = $state(untrack(() => profile.group ?? ''));
  let draft = $state('');
  let groupErr = $state<ReturnType<typeof groupErrorKey> | null>(null);
  let tagErr = $state<ReturnType<typeof tagErrorKey> | null>(null);
  let focusInput: HTMLInputElement | undefined = $state();

  const tags = $derived(profile.tags);
  const groups = $derived(knownGroups(app.profiles));
  const suggestions = $derived(knownTags(app.profiles.filter((p) => p.id !== profile.id)).filter((k) => !tags.some((x) => x.toLowerCase() === k.tag.toLowerCase())).slice(0, 8));

  async function saveGroup() {
    const r = checkGroup(group);
    if (!r.ok) { groupErr = groupErrorKey(r.error); return; }
    groupErr = null;
    if (r.group === profile.group) return;
    try { await actions.updateProfile(profile.id, { group: r.group }); onsaved?.(); } catch (e) { toast(errorText(e), 'error'); }
  }
  async function setTags(next: string[]) {
    try { await actions.updateProfile(profile.id, { tags: next }); onsaved?.(); } catch (e) { toast(errorText(e), 'error'); }
  }
  async function add(raw = draft) {
    const r = checkTag(raw, tags);
    if (!r.ok) { tagErr = r.error === 'empty' && !raw.trim() ? null : tagErrorKey(r.error); return; }
    tagErr = null; draft = '';
    await setTags([...tags, r.tag]);
    focusInput?.focus();
  }
  async function remove(tag: string) { tagErr = null; await setTags(tags.filter((x) => x !== tag)); }
  function key(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ',') { e.preventDefault(); e.stopPropagation(); void add(); }
    else if (e.key === 'Backspace' && !draft && tags.length) void remove(tags[tags.length - 1]);
  }
  const full = $derived(tags.length >= TAGS_MAX);
</script>

<div class="set-row stack" id="set-grouptags">
  <label for="gt-group">{t('group.label')}</label>
  <input id="gt-group" class="field" list="gt-groups" bind:value={group} maxlength={GROUP_MAX + 8} placeholder={t('group.placeholder')} autocomplete="off" aria-invalid={!!groupErr}
    aria-describedby="gt-group-h" onblur={saveGroup} onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); e.currentTarget.blur(); } }} />
  <datalist id="gt-groups">{#each groups as g (g)}<option value={g}></option>{/each}</datalist>
  <div id="gt-group-h" class="hint">{t('group.hint')}</div>
  {#if groupErr}<div class="err" role="alert">{t(groupErr)}</div>{/if}
</div>

<div class="set-row stack" id="set-tags">
  <div class="head"><label for="gt-tag">{t('tags.label')}</label><span class="hint mono" aria-live="polite">{t('tags.count', { n: tags.length })}</span></div>
  {#if tags.length}
    <ul class="chips" aria-label={t('tags.label')}>
      {#each tags as tg (tg)}
        <li class="chip tag"><TagIcon weight="bold" size={12} aria-hidden="true" />{tg}
          <button type="button" class="x" aria-label={t('tags.remove', { tag: tg })} onclick={() => remove(tg)}><XIcon weight="bold" size={12} aria-hidden="true" /></button></li>
      {/each}
    </ul>
  {/if}
  <div class="add">
    <input id="gt-tag" class="field" bind:this={focusInput} bind:value={draft} maxlength={TAG_MAX + 8} placeholder={t('tags.placeholder')} autocomplete="off" disabled={full}
      aria-invalid={!!tagErr} aria-describedby="gt-tag-h" onkeydown={key} />
    <button class="btn" type="button" disabled={full || !draft.trim()} onclick={() => add()}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('tags.add')}</button>
  </div>
  <div id="gt-tag-h" class="hint">{full ? t('tags.err.limit') : t('tags.hint')}</div>
  {#if tagErr}<div class="err" role="alert">{t(tagErr)}</div>{/if}
  {#if suggestions.length && !full}
    <div class="sug" aria-label={t('tags.used')}>
      <span class="hint">{t('tags.used')}</span>
      {#each suggestions as s (s.tag)}<button type="button" class="chip pick" onclick={() => add(s.tag)}>{s.tag}</button>{/each}
    </div>
  {/if}
</div>

<style>
  label { font-weight: 500; }
  .head { display: flex; justify-content: space-between; gap: 12px; }
  .chips { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 6px; }
  .tag { color: var(--paper); padding-right: 2px; }
  .x { width: 20px; height: 20px; display: grid; place-items: center; border-radius: 2px; color: var(--paper-mute); }
  .x:hover { color: var(--tomato); background: var(--ink-3); }
  .add { display: flex; gap: 8px; }
  .add .field { flex: 1; }
  .sug { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .pick { cursor: default; }
  .pick:hover { border-color: var(--accent-fill); color: var(--paper); }
</style>
