import fs from 'node:fs';
import os from 'node:os';
import nodePath from 'node:path';

const temporaryDirectory = fs.mkdtempSync(nodePath.join(os.tmpdir(), 'rolldown-preload-spoof-'));
const markerPath = nodePath.join(temporaryDirectory, 'preload-completed');
const originalAvailableParallelism = os.availableParallelism;
const siblingEntered = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT);
os.availableParallelism = () => 2;
process.env.ROLLDOWN_PRELOAD_BOUNDARY_MARKER = markerPath;

let bundle;
try {
  const { rolldown } = await import('rolldown');
  const { defineParallelPlugin } = await import('rolldown/experimental');
  const plugin = defineParallelPlugin(nodePath.join(import.meta.dirname, 'hanging-plugin.mjs'));

  try {
    bundle = await rolldown({
      cwd: import.meta.dirname,
      input: 'input.js',
      plugins: [plugin({ siblingEntered })],
    });
    await bundle.generate();
    throw new Error('parallel worker bootstrap unexpectedly succeeded');
  } catch (error) {
    if (!String(error?.message ?? error).includes('sentinel parallel bootstrap failure')) {
      throw error;
    }
  }

  if (fs.existsSync(markerPath)) {
    throw new Error('parallel worker executed an inherited preload');
  }
  console.log('parallel worker preload injection stripped');
} finally {
  await bundle?.close().catch(() => {});
  os.availableParallelism = originalAvailableParallelism;
  delete process.env.ROLLDOWN_PRELOAD_BOUNDARY_MARKER;
  fs.rmSync(temporaryDirectory, { force: true, recursive: true });
}
