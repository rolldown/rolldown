// Parallel-plugin implementation for the bridge-parallel variant, dynamically
// imported by each rolldown worker. One BenchVizeTransformer per worker —
// each worker dlopens the Vize cdylib independently.

import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const binding = require('../../../packages/rolldown/src/binding.cjs');

export default defineParallelPluginImplementation((_options, _context) => {
  const transformer = new binding.BenchVizeTransformer();
  return {
    name: 'vize-bench-bridge-parallel',
    transformNativeBridge(handle) {
      try {
        return transformer.transformNative(handle);
      } catch {
        return undefined;
      }
    },
  };
});
