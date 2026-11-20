<script lang="ts">
  // The one place that shows an instance image: custom/modpack image, chosen block, auto block, else the MangoClient icon. Never letters.
  import Tile from './Tile.svelte';
  import BlockIcon from './BlockIcon.svelte';
  import { iconTint, resolveInstanceIcon } from '../lib/instanceIcon';
  import { blocks } from '../lib/stores.svelte';
  import type { ProfileView } from '../lib/types';

  interface Props {
    profile?: Pick<ProfileView, 'id' | 'color' | 'coverUrl' | 'icon' | 'autoBlock' | 'loader'> | null;
    /** Used when `profile` is unknown (deleted instance). */
    id?: string;
    size?: number;
    radius?: number;
    notch?: number;
    fill?: boolean;
    alt?: string;
  }
  let { profile = null, id = '', size = 40, radius = 6, notch = 0, fill = false, alt = '' }: Props = $props();

  const tint = $derived(iconTint(profile));
  const r = $derived(resolveInstanceIcon(profile, blocks.status));
  let failedBlock = $state<string | null>(null);
  const useBlock = $derived(r.kind === 'block' && failedBlock !== r.block.id);
</script>

{#if r.kind === 'block' && useBlock}
  <BlockIcon faces={r.block.faces} {size} {fill} {radius} {notch} {tint} label={alt} onfail={() => (failedBlock = r.block.id)} />
{:else}
  <Tile kind="profile" id={profile?.id ?? id} src={r.kind === 'image' ? r.src : null} {tint} {size} {radius} {notch} {fill} {alt} />
{/if}
