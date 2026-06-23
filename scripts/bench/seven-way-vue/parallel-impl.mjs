// Parallel-plugin implementation for the bridge-parallel variant, dynamically
// imported by each rolldown worker. One BenchVizeTransformer per worker —
// each worker dlopens the Vize cdylib independently.

import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const binding = require('../../../packages/rolldown/src/binding.cjs');

const __dirname = dirname(fileURLToPath(import.meta.url));
const VIZE_LIB_PATH = process.env.VIZE_LIB_PATH ?? resolve(
  __dirname,
  'native/target/release/libbench_vize_sfc_lib.dylib',
);

export default defineParallelPluginImplementation((_options, _context) => {
  const transformer = new binding.BenchVizeTransformer(VIZE_LIB_PATH);
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
