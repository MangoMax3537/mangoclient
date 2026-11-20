<script lang="ts">
  // The one toggle switch. The real checkbox (role=switch) fills the whole 44x24 track, so a click anywhere on it toggles, Space and Enter
  // work from the keyboard, and a surrounding <label> (see SettingRow) toggles it from its text as well.
  interface Props {
    checked: boolean;
    /** Called with the new state; `e.currentTarget` is the input (a caller may veto by setting `checked` back). */
    onchange: (on: boolean, e: Event & { currentTarget: HTMLInputElement }) => void;
    id?: string;
    /** Accessible name when no <label> wraps or points at the switch. */
    label?: string;
    disabled?: boolean;
    describedby?: string;
  }
  let { checked, onchange, id, label, disabled = false, describedby }: Props = $props();
</script>

<span class="toggle">
  <input type="checkbox" role="switch" {id} {checked} {disabled} aria-label={label} aria-describedby={describedby}
    onchange={(e) => onchange(e.currentTarget.checked, e)}
    onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); e.currentTarget.click(); } }} />
  <span class="track" aria-hidden="true"><span class="thumb"></span></span>
</span>

<style>
  .toggle { position: relative; display: inline-block; width: 44px; height: 24px; flex: none; }
  input { position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; opacity: 0; cursor: default; z-index: 1; appearance: none; }
  input:disabled { cursor: default; }
  .track { position: absolute; inset: 0; background: var(--ink-4); border: 1px solid var(--line); border-radius: 4px; transition: background var(--dur) var(--ease), border-color var(--dur) var(--ease); pointer-events: none; }
  .thumb { position: absolute; left: 3px; top: 3px; width: 16px; height: 16px; border-radius: 2px; background: var(--paper-dim); transition: transform var(--dur) var(--ease), background var(--dur) var(--ease); }
  input:hover:not(:disabled) + .track { border-color: var(--paper-mute); }
  input:checked + .track { background: var(--accent-fill); border-color: var(--accent-fill); }
  input:checked + .track .thumb { transform: translateX(20px); background: var(--on-fill); }
  input:focus-visible + .track { outline: 2px solid var(--focus); outline-offset: 2px; }
  input:disabled + .track { opacity: 0.45; }
</style>
