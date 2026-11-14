import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputAsset } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// https://github.com/rolldown/rolldown/issues/11690
// Relative `sources` in a transform hook's map are relative to the module, not to the cwd.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform(code, id) {
          if (!id.endsWith('dep.js')) {
            return null;
          }
          return {
            code,
            map: {
              version: 3,
              names: [],
              sources: ['../src/a.ts', '../src/b.ts'],
              sourcesContent: ["console.log('dep');\n", ''],
              mappings: 'AAAA,QAAQ,CAAC,GAAG,CCAC',
            },
          };
        },
      },
    ],
    output: {
      dir: 'dist/deep',
      sourcemap: true,
    },
  },
  afterTest: (output) => {
    const map = JSON.parse(getOutputAsset(output)[0].source as string);
    const outDir = path.join(import.meta.dirname, 'dist/deep');
    const sources = (map.sources as string[]).map((source) => path.resolve(outDir, source));
    expect(sources).toContain(path.join(import.meta.dirname, 'src/a.ts'));
    expect(sources).not.toContain(path.join(import.meta.dirname, '../src/a.ts'));
  },
});
