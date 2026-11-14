import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// A merged-away order-wrap entry needs no facade: the emitted `import()` names the
// chunk hosting the entry's body and carries the trigger in its `.then`
// (`import("./chunks/dyn.js").then((n) => (n.init(), n.ns))`), and `dynamicImports`
// must name that same file. See internal-docs/code-splitting/design.md
// ("Trigger placement").
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
    expect(a.code).toContain('import("./chunks/dyn.js")');
    expect(a.dynamicImports).toStrictEqual(['chunks/dyn.js']);
    expect(output.output.some((item) => item.fileName === 'chunks/target.js')).toBe(false);
  },
});
