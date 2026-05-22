import { defineTest } from 'rolldown-tests';
import { viteWasmFallbackPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [viteWasmFallbackPlugin()],
  },
  catchError(err) {
    expect((err as Error).message).includes('[UNRESOLVED_IMPORT]');
  },
  afterTest() {
    expect.unreachable('viteWasmFallbackPlugin');
  },
});
