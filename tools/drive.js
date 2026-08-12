/**
 * Dev harness: drives the running launcher over the Chrome DevTools Protocol so
 * the UI can be exercised and screenshotted without a real pointer.
 *
 *   node tools/drive.js <script.json>
 *
 * Start the app with --remote-debugging-port=9222 first.
 */
'use strict';
const fs = require('fs');
const path = require('path');

const PORT = process.env.CDP_PORT || 9222;
const OUT = process.env.SHOT_DIR || '/tmp/mangoshots';

async function listTargets() {
  const res = await fetch(`http://127.0.0.1:${PORT}/json/list`);
  return res.json();
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    let id = 0;
    const pending = new Map();
    ws.addEventListener('open', () => resolve({
      send(method, params = {}) {
        const msgId = ++id;
        ws.send(JSON.stringify({ id: msgId, method, params }));
        return new Promise((res, rej) => pending.set(msgId, { res, rej }));
      },
      close: () => ws.close(),
    }));
    ws.addEventListener('error', reject);
    ws.addEventListener('message', (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        if (msg.error) rej(new Error(msg.error.message));
        else res(msg.result);
      }
    });
  });
}

async function main() {
  const steps = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  fs.mkdirSync(OUT, { recursive: true });

  const targets = await listTargets();
  const page = targets.find((t) => t.type === 'page');
  if (!page) throw new Error('No page target. Is the app running with --remote-debugging-port?');
  const cdp = await connect(page.webSocketDebuggerUrl);

  await cdp.send('Runtime.enable');
  await cdp.send('Page.enable');

  // Surface renderer errors instead of letting them hide behind a blank view.
  const errors = [];
  await cdp.send('Log.enable').catch(() => {});

  for (const step of steps) {
    if (step.eval) {
      const r = await cdp.send('Runtime.evaluate', {
        expression: step.eval,
        awaitPromise: true,
        returnByValue: true,
      });
      if (r.exceptionDetails) {
        console.log(`EVAL ERROR: ${step.eval} -> ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
        errors.push(step.eval);
      } else if (r.result?.value !== undefined) {
        console.log(`${step.label || step.eval} => ${JSON.stringify(r.result.value).slice(0, 400)}`);
      }
    }
    if (step.wait) await new Promise((r) => setTimeout(r, step.wait));
    if (step.shot) {
      const { data } = await cdp.send('Page.captureScreenshot', { format: 'png' });
      fs.writeFileSync(path.join(OUT, `${step.shot}.png`), Buffer.from(data, 'base64'));
      console.log(`shot: ${step.shot}`);
    }
  }

  cdp.close();
  if (errors.length) process.exitCode = 1;
}

main().catch((err) => { console.error(err.message); process.exit(1); });
