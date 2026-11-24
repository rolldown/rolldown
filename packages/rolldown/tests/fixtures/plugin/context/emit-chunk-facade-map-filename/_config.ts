import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const fileName = 'fixed-a.js';
const mapFileName = 'fixed-a.js.map';
const codeSplitting = {
  includeDependenciesRecursively: false,
  groups: [{ name: 'group', test: /[\\/][ab]\.js$/ }],
};
let chunkReferenceId: string;
let assetReferenceId: string;

export default defineTest({
  config: {
    preserveEntrySignatures: 'allow-extension',
    output: [
      { format: 'es', codeSplitting, sourcemap: true },
      { format: 'cjs', codeSplitting, sourcemap: true },
    ],
    plugins: [
      {
        name: 'emit-facade-and-asset',
        buildStart() {
          chunkReferenceId = this.emitFile({
            type: 'chunk',
            id: './a.js',
            fileName,
            preserveSignature: 'strict',
          });
          this.emitFile({ type: 'chunk', id: './b.js' });
          assetReferenceId = this.emitFile({
            type: 'asset',
            fileName: mapFileName,
            source: 'asset',
          });
        },
        generateBundle(_, bundle) {
          expect(this.getFileName(chunkReferenceId)).toBe(fileName);
          expect(this.getFileName(assetReferenceId)).toBe(mapFileName);
          expect(bundle[fileName]).toMatchObject({
            type: 'chunk',
            exports: ['a'],
            sourcemapFileName: null,
          });
          expect(bundle[mapFileName]).toMatchObject({ type: 'asset', source: 'asset' });
        },
      },
    ],
  },
});
