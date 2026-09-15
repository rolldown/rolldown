// Regression test for #8411 and #8747, WASI only.
//
// The wasm binding refcounts the shared tokio runtime. Before the fix,
// `RolldownBuild.close()` and `Watcher.close()` released the runtime without a
// matching acquire, and `DevEngine` never acquired at all. The first `close()`
// in the process then tore the runtime down under every later consumer, and
// the next binding call crashed with "Access tokio runtime failed in spawn".
// On native targets the runtime functions are no-ops, so this can only be
// caught by running against the WASI binding.
//
// This file is a plain node script wired into the WASI CI lane. It must set
// NAPI_RS_FORCE_WASI before importing rolldown, and `error` mode is required:
// with `true`, a missing wasi build silently falls back to the native binding
// and the test would pass without testing anything.
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { createRequire } from 'node:module';

process.env.NAPI_RS_FORCE_WASI = 'error';

const { rolldown, watch } = await import('rolldown');
const { dev } = await import('rolldown/experimental');

const require = createRequire(import.meta.url);
if (!Object.keys(require.cache).some((k) => k.includes('rolldown-binding.wasi.cjs'))) {
  console.error('[wasi-runtime] the WASI binding was not loaded');
  process.exit(1);
}

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-wasi-runtime-'));
function makeFixture(name) {
  const dir = path.join(root, name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'entry.js'), 'export const value = 1;\n');
  return dir;
}

async function buildAndClose(dir) {
  const bundle = await rolldown({ input: path.join(dir, 'entry.js'), cwd: dir });
  await bundle.generate({ format: 'esm' });
  await bundle.close();
}

// #8411: a closed build (Vite bundling vite.config.ts) followed by a DevEngine.
{
  const dir = makeFixture('dev-after-close');
  await buildAndClose(dir);
  const engine = await dev(
    { input: path.join(dir, 'entry.js'), cwd: dir },
    { dir: path.join(dir, 'dist') },
    {},
  );
  await engine.run();
  await engine.ensureCurrentBuildFinish();
  await engine.close();
  console.log('[wasi-runtime] dev engine after build close: ok');
}

// #8747: a closed build followed by watch mode.
{
  const dir = makeFixture('watch-after-close');
  await buildAndClose(dir);
  const watcher = watch({
    input: path.join(dir, 'entry.js'),
    cwd: dir,
    output: { dir: path.join(dir, 'dist') },
  });
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('timed out waiting for BUNDLE_END')), 60_000);
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        clearTimeout(timer);
        resolve();
      } else if (event.code === 'ERROR') {
        clearTimeout(timer);
        reject(event.error);
      }
    });
  });
  await watcher.close();
  console.log('[wasi-runtime] watch after build close: ok');
}

// The invariant behind both fixes: closing one consumer must not tear the
// runtime down under another consumer that is still alive.
{
  const dir = makeFixture('close-while-dev-alive');
  const engine = await dev(
    { input: path.join(dir, 'entry.js'), cwd: dir },
    { dir: path.join(dir, 'dist') },
    {},
  );
  await engine.run();
  await buildAndClose(makeFixture('unrelated-build'));
  await engine.ensureCurrentBuildFinish();
  await engine.close();
  console.log('[wasi-runtime] build close while dev engine alive: ok');
}

fs.rmSync(root, { recursive: true, force: true });
console.log('[wasi-runtime] PASS');
process.exit(0);
