import path from 'node:path';
import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// A Vue file and its script part both get the name `Comp.vue`, as in Rollup. Chunks get their
// names in the default order: user-defined entries first, then the other chunks in execution
// order. The script part runs first, so it gets `Comp.vue.js`, and the Vue file gets
// `Comp2.vue.js`. Rollup gives the same file names when the Vue file has no side effects, as here.
// `main.js` imports the Vue file chunk, but Rollup's `main.js` imports the script part directly:
// that is a known difference in linking, not in naming.
const compId = path.join(import.meta.dirname, 'Comp.vue');
const scriptId = `${compId}?vue&type=script&setup=true&lang.ts`;

export default defineTest({
  sequential: true,
  config: {
    output: {
      dir: 'dist',
      preserveModules: true,
      entryFileNames: '[name].js',
    },
    plugins: [
      {
        name: 'virtual-vue-file',
        resolveId(id) {
          if (id === './Comp.vue') return compId;
          if (id === 'comp-script') return scriptId;
        },
        load(id) {
          if (id === compId) {
            return `import { foo } from 'comp-script';\nexport default foo;`;
          }
          if (id === scriptId) {
            return `export const foo = 1;\nglobalThis.nameVueScriptPart.push('script');`;
          }
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    const role = (chunk: OutputChunk) =>
      chunk.facadeModuleId?.includes('?vue&type=script')
        ? 'script'
        : chunk.facadeModuleId?.endsWith('Comp.vue')
          ? 'wrapper'
          : chunk.name;
    expect(
      Object.fromEntries(chunks.map((chunk) => [role(chunk), [chunk.fileName, chunk.imports]])),
    ).toStrictEqual({
      main: ['main.js', ['Comp2.vue.js']],
      wrapper: ['Comp2.vue.js', ['Comp.vue.js']],
      script: ['Comp.vue.js', []],
    });
    await import('./_test.mjs');
  },
});
