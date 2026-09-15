import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const expectedImports = ['mapped/star-first.js', 'mapped/named-second.js', 'mapped/both.js'];

export default defineTest({
  config: {
    external: ['star-first', 'named-second', 'both'],
    output: {
      paths: {
        'star-first': 'mapped/star-first.js',
        'named-second': 'mapped/named-second.js',
        both: 'mapped/both.js',
      },
    },
    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (_code, chunk) => {
          expect(chunk.imports).toStrictEqual(expectedImports);
        },
        generateBundle: (_options, bundle) => {
          const chunk = bundle['main.js'] as OutputChunk;
          expect(chunk.imports).toStrictEqual(expectedImports);
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunk = output.output[0] as OutputChunk;
    expect(chunk.code).toContain('export * from "mapped/star-first.js"');
    expect(chunk.code).toContain('import { named } from "mapped/named-second.js"');
    expect(chunk.code).toContain('export * from "mapped/both.js"');
    expect(chunk.code).toContain('import { named as alsoNamed } from "mapped/both.js"');
  },
});
