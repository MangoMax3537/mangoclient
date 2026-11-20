// Markdown-lite for untrusted Modrinth project bodies. Output is a plain data tree that Svelte renders as text nodes:
// no innerHTML, no raw HTML, no links or images are ever created from the source.
export type Inline = { kind: 'text' | 'bold' | 'italic' | 'code'; text: string };
export type Block =
  | { kind: 'h'; level: 1 | 2 | 3; inline: Inline[] }
  | { kind: 'p'; inline: Inline[] }
  | { kind: 'ul'; items: Inline[][] }
  | { kind: 'code'; text: string };

const MAX_BLOCKS = 400;

function clean(s: string): string {
  return s
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<[^>\n]{1,300}>/g, '') // raw HTML tags dropped, text kept
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1') // images -> alt text
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1') // links -> label only
    .replace(/<?https?:\/\/\S+>?/g, ''); // bare urls dropped
}

export function inlineParse(src: string): Inline[] {
  const out: Inline[] = [];
  const re = /(`[^`]+`|\*\*[^*]+\*\*|__[^_]+__|\*[^*\s][^*]*\*)/g;
  let last = 0;
  for (let m = re.exec(src); m; m = re.exec(src)) {
    if (m.index > last) out.push({ kind: 'text', text: src.slice(last, m.index) });
    const tok = m[0];
    if (tok.startsWith('`')) out.push({ kind: 'code', text: tok.slice(1, -1) });
    else if (tok.startsWith('**') || tok.startsWith('__')) out.push({ kind: 'bold', text: tok.slice(2, -2) });
    else out.push({ kind: 'italic', text: tok.slice(1, -1) });
    last = m.index + tok.length;
  }
  if (last < src.length) out.push({ kind: 'text', text: src.slice(last) });
  return out;
}

export function markdownLite(src: string): Block[] {
  const blocks: Block[] = [];
  const lines = src.replace(/\r\n?/g, '\n').split('\n');
  let para: string[] = [];
  let list: Inline[][] | null = null;
  const flushPara = () => {
    const text = clean(para.join(' ')).replace(/\s+/g, ' ').trim();
    if (text) blocks.push({ kind: 'p', inline: inlineParse(text) });
    para = [];
  };
  const flushList = () => { if (list?.length) blocks.push({ kind: 'ul', items: list }); list = null; };
  for (let i = 0; i < lines.length && blocks.length < MAX_BLOCKS; i++) {
    const line = lines[i];
    if (line.trimStart().startsWith('```')) {
      flushPara(); flushList();
      const buf: string[] = [];
      for (i++; i < lines.length && !lines[i].trimStart().startsWith('```'); i++) buf.push(lines[i]);
      blocks.push({ kind: 'code', text: buf.join('\n') });
      continue;
    }
    const h = /^(#{1,6})\s+(.*)$/.exec(line);
    if (h) {
      flushPara(); flushList();
      const text = clean(h[2]).trim();
      if (text) blocks.push({ kind: 'h', level: Math.min(3, h[1].length) as 1 | 2 | 3, inline: inlineParse(text) });
      continue;
    }
    const li = /^\s*[-*+]\s+(.*)$/.exec(line);
    if (li) {
      flushPara();
      const text = clean(li[1]).trim();
      if (text) (list ??= []).push(inlineParse(text));
      continue;
    }
    if (!line.trim()) { flushPara(); flushList(); continue; }
    flushList();
    para.push(line.trim());
  }
  flushPara(); flushList();
  return blocks;
}
