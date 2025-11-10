import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import type { OutputChunk } from 'rolldown';

export default defineTest({
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
        generateBundle: (_options, bundle) => {
          const chunk = bundle['main.js'] as OutputChunk;
          // The imports should use the modified paths from output.paths function
          expect(chunk.imports).toStrictEqual(['react/index.js', 'vue/dist/vue.esm.js']);
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
