import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Top-level await disables common-chunk merging, so the dynamic entry keeps its
// facade chunk while the manual group hosts the entry module's body — the same
// metadata/specifier split as the order-wrap facade, without `strictExecutionOrder`.
export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      entryFileNames: '[name].js',
      chunkFileNames: 'chunks/[name].js',
      codeSplitting: {
        groups: [{ name: 'grp', test: /[\\/](?:page|lib)\.js$/ }],
      },
    },
  },
  afterTest: (output) => {
    const main = output.output.find((item) => item.type === 'chunk' && item.fileName === 'main.js');
    if (main?.type !== 'chunk') {
      throw new Error('main.js should be emitted as a chunk');
    }
    expect(main.code).toContain('import("./chunks/page.js")');
    expect(main.dynamicImports).toStrictEqual(['chunks/page.js']);
  },
});
