import { createRequire } from 'node:module';

import { expect, test } from 'vitest';

const require = createRequire(import.meta.url);
const binding = require('../src/binding.cjs');
const capabilities = binding.getRuntimeCapabilities();

test.runIf(capabilities.target !== 'wasi-threads')(
  'manual runtime lifecycle exports remain no-ops outside threaded WASI',
  () => {
    expect(binding.startAsyncRuntime()).toBeUndefined();
    expect(binding.shutdownAsyncRuntime()).toBeUndefined();
  },
);
