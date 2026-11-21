// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Console from './Console.svelte';
import { consoles, resetStores } from '../lib/stores.svelte';

beforeEach(() => { vi.useFakeTimers(); resetStores(); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });
const btn = (root: ParentNode, re: RegExp) => [...root.querySelectorAll('button')].find((b) => re.test(b.textContent ?? ''));

describe('console copy all', () => {
  it('is disabled when empty', () => {
    const target = document.createElement('div'); document.body.append(target);
    const a = mount(Console, { target, props: { profileId: 'p' } }); flushSync();
    expect((btn(target, /Alles kopieren|Copy all/) as HTMLButtonElement).disabled).toBe(true);
    unmount(a);
  });
  it('copies ALL lines as plain text, then shows Copied for about 1.5 s', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    consoles.p = Array.from({ length: 500 }, (_, i) => ({ id: i, level: 'info', line: `line ${i}` })) as never;
    const target = document.createElement('div'); document.body.append(target);
    const a = mount(Console, { target, props: { profileId: 'p' } }); flushSync();
    btn(target, /Alles kopieren|Copy all/)!.click();
    await vi.advanceTimersByTimeAsync(0); flushSync();
    expect(writeText).toHaveBeenCalledTimes(1);
    const text = writeText.mock.calls[0][0] as string;
    expect(text.split('\n')).toHaveLength(500);
    expect(text.startsWith('line 0\n')).toBe(true);
    expect(target.querySelector('[role="status"]')?.textContent).toMatch(/Kopiert|Copied/);
    await vi.advanceTimersByTimeAsync(1600); flushSync();
    expect(target.querySelector('[role="status"]')?.textContent).toBe('');
    unmount(a);
  });
  it('falls back to a textarea + execCommand when navigator.clipboard is missing', async () => {
    vi.stubGlobal('navigator', {});
    const exec = vi.fn().mockReturnValue(true);
    (document as unknown as { execCommand: unknown }).execCommand = exec;
    const { writeClipboard } = await import('../lib/clipboard');
    expect(await writeClipboard('a\nb')).toBe(true);
    expect(exec).toHaveBeenCalledWith('copy');
    expect(document.querySelector('textarea')).toBeNull();
  });
});
