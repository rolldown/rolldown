import path from 'node:path';
import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: {
      entry: './entry.js',
    },
    output: {
      dir: 'dist',
      preserveModules: true,
      preserveModulesRoot: '.',
      entryFileNames({ name }) {
        return `${name}.js`;
      },
      sourcemap: true,
      sourcemapFileNames: '[name].js.map',
    },
    plugins: [
      {
        name: 'virtual-query-string',
        resolveId(id, importer) {
          if (id === 'virtual') {
            if (!importer) return null;
            return path.join(path.dirname(importer), 'Comp.vue?vue&type=script&setup=true&lang.ts');
          }
        },
        load(id) {
          if (id.includes('Comp.vue?vue&type=script&setup=true&lang.ts')) {
            return 'export const foo = 1; console.log(foo);';
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    expect(
      Object.fromEntries(
        chunks.map((chunk) => [chunk.fileName, [chunk.name, chunk.sourcemapFileName]]),
      ),
    ).toStrictEqual({
      'Comp.vue_vue_type_script_setup_true_lang.js': [
        'Comp.vue_vue_type_script_setup_true_lang',
        'Comp.vue_vue_type_script_setup_true_lang.js.map',
      ],
      'entry.js': ['entry', 'entry.js.map'],
    });
  },
});
