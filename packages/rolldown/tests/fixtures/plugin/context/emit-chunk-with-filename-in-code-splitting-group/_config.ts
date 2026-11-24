import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const fileName = 'fixed-a.js';
const codeSplitting = {
  includeDependenciesRecursively: false,
  groups: [{ name: 'group', test: /[\\/][ab]\.js$/ }],
};
let referenceIds: string[];
let assetReferenceId: string;

export default defineTest({
  config: {
    input: 'main.js',
    preserveEntrySignatures: 'allow-extension',
    output: [
      { format: 'es', codeSplitting, assetFileNames: '[name][extname]' },
      { format: 'cjs', codeSplitting, assetFileNames: '[name][extname]', entryFileNames: fileName },
    ],
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          assetReferenceId = this.emitFile({ type: 'asset', name: fileName, source: 'asset' });
          referenceIds = [
            this.emitFile({
              type: 'chunk',
              id: './a.js',
              fileName,
              preserveSignature: 'allow-extension',
            }),
            this.emitFile({ type: 'chunk', id: './b.js', preserveSignature: 'allow-extension' }),
          ];
        },
        renderChunk(_, chunk, options) {
          if (chunk.name === 'main') {
            expect(referenceIds.map((id) => this.getFileName(id))).toEqual([fileName, fileName]);
            expect(this.getFileName(assetReferenceId)).toBe(
              options.format === 'cjs' ? 'fixed-a3.js' : 'fixed-a2.js',
            );
          }
        },
        generateBundle(options, bundle) {
          expect(referenceIds.map((id) => this.getFileName(id))).toEqual([fileName, fileName]);
          expect(bundle[fileName]).toMatchObject({
            type: 'chunk',
            name: 'group',
            exports: ['a', 'b'],
          });
          const mainFileName = options.format === 'cjs' ? 'fixed-a2.js' : 'main.js';
          expect(bundle[mainFileName]).toMatchObject({ type: 'chunk', name: 'main' });
          const assetFileName = options.format === 'cjs' ? 'fixed-a3.js' : 'fixed-a2.js';
          expect(this.getFileName(assetReferenceId)).toBe(assetFileName);
          expect(bundle[assetFileName]).toMatchObject({ type: 'asset', source: 'asset' });
          const lateAssetId = this.emitFile({
            type: 'asset',
            name: fileName,
            source: 'late asset',
          });
          expect(this.getFileName(lateAssetId)).toBe(
            options.format === 'cjs' ? 'fixed-a4.js' : 'fixed-a3.js',
          );
        },
      },
    ],
  },
});
