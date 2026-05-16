import path from 'node:path';
import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: {
      index: './entry.js',
    },
    output: {
      dir: 'dist',
      entryFileNames({ name }) {
        return `${name}.js`;
      },
      preserveModules: true,
    },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(id) {
          if (id === './components/TestComp.vue') {
            return path.join(
              import.meta.dirname,
              'components/TestComp.vue?vue&type=script&setup=true&lang',
            );
          }
        },
        load(id) {
          if (id.includes('TestComp.vue?vue&type=script&setup=true&lang')) {
            return 'console.log()';
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    const entryChunk = output.output.find(
      (item): item is OutputChunk => item.type === 'chunk' && item.fileName === 'index.js',
    );
    expect(entryChunk?.code).toMatchInlineSnapshot(`
      "import "./components/TestComp.vue_vue_type_script_setup_true_lang.js";
      "
    `);

    const testCompChunk = output.output.find((item) =>
      item.fileName.includes('TestComp.vue_vue_type_script_setup_true_lang.js'),
    );
    expect(testCompChunk?.fileName).toMatchInlineSnapshot(
      `"components/TestComp.vue_vue_type_script_setup_true_lang.js"`,
    );
  },
});
