import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// A two-argument `import()` of a bundled module is rewritten like any other call site, so
// `dynamicImports` names the emitted chunk and the emitted call points at that same file
// instead of the specifier the source wrote.
export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      entryFileNames: '[name].js',
      chunkFileNames: 'chunks/[name]-[hash].js',
    },
  },
  afterTest: (output) => {
    const main = output.output.find((item) => item.type === 'chunk' && item.fileName === 'main.js');
    if (main?.type !== 'chunk') {
      throw new Error('main.js should be emitted as a chunk');
    }
    expect(main.dynamicImports).toHaveLength(1);
    const [page] = main.dynamicImports;
    expect(page).toMatch(/^chunks\/page-[\w-]+\.js$/);
    expect(main.code).toContain(`import("./${page}")`);
    expect(main.code).not.toContain('./page.js');
  },
});
