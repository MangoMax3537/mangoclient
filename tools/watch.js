/** Poll the running launcher's UI state until the game starts or fails. */
'use strict';
const PORT = process.env.CDP_PORT || 9222;

async function connect() {
  const targets = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = targets.find((t) => t.type === 'page');
  if (!page) throw new Error('no page target');
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    let id = 0;
    const pending = new Map();
    ws.addEventListener('open', () => resolve({
      send(method, params = {}) {
        const msgId = ++id;
        ws.send(JSON.stringify({ id: msgId, method, params }));
        return new Promise((res, rej) => pending.set(msgId, { res, rej }));
      },
    }));
    ws.addEventListener('error', reject);
    ws.addEventListener('message', (e) => {
      const m = JSON.parse(e.data);
      if (m.id && pending.has(m.id)) {
        const { res, rej } = pending.get(m.id);
        pending.delete(m.id);
        m.error ? rej(new Error(m.error.message)) : res(m.result);
      }
    });
  });
}

const PROBE = `(() => {
  const out = document.querySelector('#console-out');
  const lines = [...out.childNodes].slice(-6).map(n => n.textContent.trim()).filter(Boolean);
  return JSON.stringify({
    progress: document.querySelector('#progress-text').textContent,
    pct: document.querySelector('#progress-pct').textContent,
    hidden: document.querySelector('#launch-progress').hidden,
    play: document.querySelector('#play-label').textContent,
    tail: lines,
  });
})()`;

(async () => {
  const cdp = await connect();
  await cdp.send('Runtime.enable');
  let last = '';
  const deadline = Date.now() + (Number(process.env.WATCH_MS) || 900000);

  while (Date.now() < deadline) {
    const r = await cdp.send('Runtime.evaluate', { expression: PROBE, returnByValue: true });
    const data = JSON.parse(r.result.value);
    const key = `${data.progress}|${data.play}|${data.tail.at(-1) || ''}`;
    if (key !== last) {
      last = key;
      console.log(`[${data.pct}] ${data.progress} :: ${data.play}`);
      for (const line of data.tail.slice(-2)) console.log(`    ${line.slice(0, 200)}`);
    }
    // The launcher hides the bar and flips the button once the game is up.
    if (data.hidden && /BEENDEN|STOP/.test(data.play)) { console.log('RESULT: game running'); return; }
    if (/Launch failed|exited with code|Traceback/i.test(data.tail.join(' '))) {
      console.log('RESULT: failure'); console.log(data.tail.join('\n')); return;
    }
    await new Promise((r2) => setTimeout(r2, 5000));
  }
  console.log('RESULT: timeout');
})().catch((e) => { console.error(e.message); process.exit(1); });
