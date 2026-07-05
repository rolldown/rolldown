import nodePath from 'node:path';
import { rolldown } from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';

const plugin = defineParallelPlugin(nodePath.join(import.meta.dirname, 'delayed-plugin.mjs'));
const bundle = await rolldown({
  cwd: import.meta.dirname,
  input: 'input.js',
  plugins: [plugin({ delay: 100 })],
});
await bundle.close();
console.log('parallel worker bootstrap completed');
