import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    external: ['react'],
    output: {
      paths: {
        react: 'react/index.js',
      },
    },
    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (_code, chunk) => {
          // The imports should use the modified path from output.paths
          expect(chunk.imports).toStrictEqual(['react/index.js']);
        },
        generateBundle: (_options, bundle) => {
          const chunk = bundle['main.js'] as OutputChunk;
          // The imports should use the modified path from output.paths
          expect(chunk.imports).toStrictEqual(['react/index.js']);
        },
      },
    ],
  },
  afterTest: (output) => {
    // Verify the generated code also uses the modified path
    expect(output.output[0].code).toContain('from "react/index.js"');
  },
});
