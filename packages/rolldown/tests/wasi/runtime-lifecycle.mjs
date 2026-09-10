// Runtime lifecycle regression test, WASI only. The WASI binding runs the
// shared CurrentThread runtime, whose lifecycle is owned by the N-API
// environment rather than by JavaScript-held runtime leases (compatibility
// no-ops here), and dev()/watch() are unsupported on the WASI artifact by
// design.
//
// It must set NAPI_RS_FORCE_WASI before importing rolldown, and `error` mode is
// required: with `true`, a missing wasi build silently falls back to the native
// binding and the test would pass without testing anything.
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

async function build(dir) {
  const bundle = await rolldown({ input: path.join(dir, 'entry.js'), cwd: dir });
  await bundle.generate({ format: 'esm' });
  return bundle;
}

// 1. Repeated sequential build+close: closing one consumer must leave the
//    shared runtime usable for the next.
{
  for (let i = 0; i < 3; i++) {
    const bundle = await build(makeFixture(`sequential-${i}`));
    await bundle.close();
  }
  console.log('[wasi-runtime] sequential build+close: ok');
}

// 2. Two consumers alive at once; closing one must not tear the runtime down
//    under the other (the #8411/#8747 invariant, via the WASI-supported API).
{
  const first = await build(makeFixture('concurrent-a'));
  const second = await build(makeFixture('concurrent-b'));
  await first.close();
  await second.close();
  // A fresh build after both closes must still succeed.
  const third = await build(makeFixture('concurrent-c'));
  await third.close();
  console.log('[wasi-runtime] close-while-another-alive: ok');
}

// 3. dev() is unsupported on the WASI artifact and must reject synchronously via
//    the capability contract rather than spawning onto the runtime.
{
  const dir = makeFixture('unsupported-dev');
  let devRejected = false;
  try {
    await dev({ input: path.join(dir, 'entry.js'), cwd: dir }, { dir: path.join(dir, 'dist') }, {});
  } catch (error) {
    if (error?.code !== 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE') throw error;
    devRejected = true;
  }
  if (!devRejected) {
    console.error('[wasi-runtime] dev() should be unsupported on WASI but did not reject');
    process.exit(1);
  }
  console.log('[wasi-runtime] dev() rejected as unsupported: ok');
}

// 4. watch() is unsupported on every WASI artifact and reports it through its
//    normal ERROR event lifecycle instead of building.
{
  const dir = makeFixture('unsupported-watch');
  const watcher = watch({
    input: path.join(dir, 'entry.js'),
    cwd: dir,
    output: { dir: path.join(dir, 'dist') },
  });
  const watchError = await new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error('timed out waiting for the unsupported-watch ERROR event')),
      60_000,
    );
    watcher.on('event', (event) => {
      if (event.code === 'ERROR') {
        clearTimeout(timer);
        resolve(event.error);
      } else if (event.code === 'BUNDLE_END') {
        clearTimeout(timer);
        reject(new Error('watch() unexpectedly built on WASI instead of reporting unsupported'));
      }
    });
  });
  await watcher.close();
  if (!watchError) {
    console.error('[wasi-runtime] watch() should report an unsupported-runtime error on WASI');
    process.exit(1);
  }
  console.log('[wasi-runtime] watch() reported unsupported via ERROR event: ok');
}

fs.rmSync(root, { recursive: true, force: true });
console.log('[wasi-runtime] PASS');
process.exit(0);
