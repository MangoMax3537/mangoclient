import { describe, expect, it } from 'vitest';
import { markdownLite } from './markdown';
import { logLevel } from './logs';

describe('markdownLite', () => {
  it('parses headings, lists, code and inline marks', () => {
    const b = markdownLite('# Title\n\nHello **bold** and `code`.\n\n- one\n- two\n\n```\nraw <b>\n```');
    expect(b.map((x) => x.kind)).toEqual(['h', 'p', 'ul', 'code']);
    expect(b[1]).toMatchObject({ kind: 'p', inline: [{ kind: 'text' }, { kind: 'bold', text: 'bold' }, { kind: 'text' }, { kind: 'code', text: 'code' }, { kind: 'text' }] });
    expect((b[2] as { items: unknown[] }).items).toHaveLength(2);
  });
  it('never produces html, links or images from untrusted text', () => {
    const b = markdownLite('<script>alert(1)</script><img src=x onerror=alert(1)>\n\n![pic](https://evil/x.png) [click](javascript:alert(1)) https://evil.example/x');
    const json = JSON.stringify(b);
    expect(json).not.toContain('<script');
    expect(json).not.toContain('onerror');
    expect(json).not.toContain('evil');
    expect(json).not.toContain('javascript:');
    expect(b.every((x) => ['h', 'p', 'ul', 'code'].includes(x.kind))).toBe(true);
  });
  it('bounds output size', () => {
    expect(markdownLite('# x\n'.repeat(5000)).length).toBeLessThanOrEqual(400);
  });
});

describe('logLevel', () => {
  it('classifies', () => {
    expect(logLevel('[12:00:00] [Render thread/ERROR]: boom')).toBe('error');
    expect(logLevel('[12:00:00] [main/WARN]: hmm')).toBe('warn');
    expect(logLevel('\tat net.minecraft.Foo.bar(Foo.java:1)')).toBe('error');
    expect(logLevel('Caused by: java.lang.X')).toBe('error');
    expect(logLevel('[12:00:00] [main/INFO]: ok')).toBe('info');
  });
});
