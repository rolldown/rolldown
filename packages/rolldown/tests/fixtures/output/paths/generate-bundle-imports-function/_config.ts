import type { OutputChunk, RenderedChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  retry: 3, // FIXME: this test is flaky (https://github.com/rolldown/rolldown/issues/6737)
  config: {
    external: ['react', 'vue'],
    output: {
      paths: (id) => {
        if (id === 'react') {
          return 'react/index.js';
        }
        if (id === 'vue') {
          return 'vue/dist/vue.esm.js';
        }
        return id;
      },
    },
    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (_code, chunk) => {
          // The imports should use the modified paths from output.paths function
          expect(chunk.imports).toStrictEqual([
            'react/index.js',
            'vue/dist/vue.esm.js',
          ]);
        },
        generateBundle: (_options, bundle) => {
          const chunk = bundle['main.js'] as OutputChunk;
          // The imports should use the modified paths from output.paths function
          expect(chunk.imports).toStrictEqual([
            'react/index.js',
            'vue/dist/vue.esm.js',
          ]);
        },
      },
    ],
  },
  afterTest: (output) => {
    // Verify the generated code also uses the modified paths
    expect(output.output[0].code).toContain('from "react/index.js"');
    expect(output.output[0].code).toContain('from "vue/dist/vue.esm.js"');
  },
});
