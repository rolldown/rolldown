import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    transform: {
      define: {
        'process.env.NODE_ENV': '"production"',
      },
    },
  },
  afterTest(output) {
    // The lazy chunk should NOT be emitted because:
    // 1. TanStackRouterDevtoolsInProd is unused (tree-shaken)
    // 2. Devtools.TanStackRouterDevtools (namespace member access) should be side-effect-free
    // 3. So the TanStackRouterDevtools class (which contains import('./lazy.js')) is not included
    const lazyChunk = output.output.find(
      (item) => item.type === 'chunk' && item.fileName.includes('lazy'),
    );
    expect(lazyChunk).toBeUndefined();
  },
});
