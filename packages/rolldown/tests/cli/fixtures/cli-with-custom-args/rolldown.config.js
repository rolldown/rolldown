import assert from 'node:assert';
import { defineConfig } from 'rolldown';

export default defineConfig((args) => {
  assert.strictEqual(args.customArg, 'customValue');
  return {
    input: './index.js',
  };
});
