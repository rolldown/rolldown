import path from 'node:path';
import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// A Vue file and its script part both get the name `Comp.vue`, as in Rollup. The Vue file is a
// user-defined entry, so it gets its name first and keeps `Comp.vue.js`.
const compId = path.join(import.meta.dirname, 'Comp.vue');
const scriptId = `${compId}?vue&type=script&setup=true&lang.ts`;

export default defineTest({
  sequential: true,
  config: {
    input: './Comp.vue',
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
            return `import { foo } from 'comp-script';\nglobalThis.nameVueScriptPartEntry.push('wrapper');\nexport default foo;`;
          }
          if (id === scriptId) {
            return `export const foo = 1;\nglobalThis.nameVueScriptPartEntry.push('script');`;
          }
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    const role = (chunk: OutputChunk) =>
      chunk.facadeModuleId?.includes('?vue&type=script') ? 'script' : 'wrapper';
    expect(
      Object.fromEntries(chunks.map((chunk) => [role(chunk), [chunk.fileName, chunk.imports]])),
    ).toStrictEqual({
      wrapper: ['Comp.vue.js', ['Comp2.vue.js']],
      script: ['Comp2.vue.js', []],
    });
    await import('./_test.mjs');
  },
});
