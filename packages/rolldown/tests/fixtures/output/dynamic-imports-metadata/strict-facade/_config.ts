import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// An order-wrap entry facade keeps `entry_module_to_entry_chunk` pointing at the
// facade chunk while the manual group hosts the entry module's body. The emitted
// `import()` names the facade, and `dynamicImports` must name the same file.
export default defineTest({
  config: {
    input: ['a.js', 'b.js'],
    experimental: {
      onDemandWrapping: true,
    },
    output: {
      strictExecutionOrder: true,
      entryFileNames: '[name].js',
      chunkFileNames: 'chunks/[name].js',
      codeSplitting: {
        groups: [{ name: 'dyn', test: /[\\/](?:target|observer)\.js$/ }],
      },
    },
  },
  afterTest: (output) => {
    const a = output.output.find((item) => item.type === 'chunk' && item.fileName === 'a.js');
    if (a?.type !== 'chunk') {
      throw new Error('a.js should be emitted as a chunk');
    }
    expect(a.code).toContain('import("./chunks/target.js")');
    expect(a.dynamicImports).toStrictEqual(['chunks/target.js']);
  },
});
