import os from 'node:os';
import nodePath from 'node:path';
import { rolldown } from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';

const plugin = defineParallelPlugin(nodePath.join(import.meta.dirname, 'hanging-plugin.mjs'));
const originalAvailableParallelism = os.availableParallelism;
const siblingEntered = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT);
os.availableParallelism = () => 2;

let bundle;
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
  console.log('hanging parallel worker terminated');
} finally {
  await bundle?.close().catch(() => {});
  os.availableParallelism = originalAvailableParallelism;
}
