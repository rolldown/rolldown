// Parallel-plugin implementation for variant 7 (bridge-parallel), dynamically
// imported by each rolldown worker. One BenchOxcTransformer per worker.

import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const binding = require('../../../packages/rolldown/src/binding.cjs');

export default defineParallelPluginImplementation((_options, _context) => {
  const transformer = new binding.BenchOxcTransformer();
  return {
    name: 'oxc-bench-bridge-parallel',
    transformNativeBridge(handle) {
      // No filter — match `builtin`'s scope (every module).
      try {
        return transformer.transformNative(handle);
      } catch {
        return undefined;
      }
    },
  };
});
