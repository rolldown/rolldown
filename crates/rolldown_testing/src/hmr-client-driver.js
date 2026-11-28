// @ts-check
// Runs the output of an HMR fixture as one client tab, driven by the Rust harness
// (`hmr_test_client.rs`). Messages go one JSON object per line: from Rust on stdin, to Rust on
// stdout with `PREFIX`. Other stdout lines are the fixture's own logs.

import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';

const PREFIX = '@@rolldown-hmr-client@@ ';

/** @param {Record<string, unknown>} message */
function send(message) {
  process.stdout.write(`${PREFIX}${JSON.stringify(message)}\n`);
}

/** @type {any[]} */
const received = [];
/** @type {((message: any) => void)[]} */
const waiting = [];
const lines = createInterface({ input: process.stdin });
lines.on('line', (line) => {
  const message = JSON.parse(line);
  const resolve = waiting.shift();
  if (resolve) resolve(message);
  else received.push(message);
});

/** @returns {Promise<any>} */
function next() {
  return received.length > 0
    ? Promise.resolve(received.shift())
    : new Promise((resolve) => waiting.push(resolve));
}

class PatchRequestFailed extends Error {}

/** @param {string} file */
function importFile(file) {
  return import(pathToFileURL(file).href);
}

for (const entry of JSON.parse(process.env.ROLLDOWN_HMR_CLIENT_ENTRIES ?? '[]')) {
  await importFile(entry);
}
// Let the timers the entries scheduled run first, as they would before the first update.
await new Promise((resolve) => setTimeout(resolve, 0));

const runtime = /** @type {any} */ (globalThis).__rolldown_runtime__;
// A dev server tells the page the generation of its output; the output code does not carry it.
runtime.setOutputGeneration(Number(process.env.ROLLDOWN_HMR_CLIENT_GENERATION ?? 0));
// The stand-in for the patch GET: Rust renders the patch and writes it to a file.
runtime.importPatch = async (/** @type {string} */ url) => {
  send({ t: 'request', query: url.slice(url.indexOf('?') + 1) });
  const reply = await next();
  if (reply.t === 'error') throw new PatchRequestFailed(reply.reason);
  return importFile(reply.file);
};

send({ t: 'ready' });
for (;;) {
  const message = await next();
  if (message.t === 'done') break;
  try {
    const result = await runtime.__testHandlePush(message);
    await runtime.__testSettle();
    send({ t: 'result', ...result });
  } catch (error) {
    if (error instanceof PatchRequestFailed) {
      send({ t: 'result', status: 'reload', reason: error.message });
    } else {
      send({ t: 'error', message: String(/** @type {any} */ (error)?.stack ?? error) });
    }
  }
}
// Let the event loop drain, so the fixture's `beforeExit` checks run.
lines.close();
process.stdin.destroy();
