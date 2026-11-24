import { defineTest } from 'rolldown-tests';
import { assert, expect } from 'vitest';

const fileName = 'chunks/fixed-a.js';
const mapFileName = `${fileName}.map`;
const codeSplitting = {
  includeDependenciesRecursively: false,
  groups: [{ name: 'group', test: /[\\/][ab]\.js$/ }],
};
let referenceIds: string[];
let assetReferenceId: string;
let mapAssetReferenceId: string;
let directoryAssetReferenceId: string;

export default defineTest({
  config: {
    input: 'main.js',
    preserveEntrySignatures: 'allow-extension',
    output: [
      { format: 'es', codeSplitting, assetFileNames: '[name][extname]', sourcemap: true },
      {
        format: 'cjs',
        codeSplitting,
        assetFileNames: '[name][extname]',
        entryFileNames: fileName,
        sourcemap: true,
      },
    ],
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          assetReferenceId = this.emitFile({ type: 'asset', name: fileName, source: 'asset' });
          mapAssetReferenceId = this.emitFile({
            type: 'asset',
            name: mapFileName,
            source: 'map asset',
          });
          directoryAssetReferenceId = this.emitFile({
            type: 'asset',
            name: 'chunks',
            source: 'directory asset',
          });
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
            const suffix = options.format === 'cjs' ? 3 : 2;
            expect(this.getFileName(assetReferenceId)).toBe(`chunks/fixed-a${suffix}.js`);
            expect(this.getFileName(mapAssetReferenceId)).toBe(`chunks/fixed-a${suffix}.js.map`);
            expect(this.getFileName(directoryAssetReferenceId)).toBe('chunks2');
          }
        },
        generateBundle(options, bundle) {
          expect(referenceIds.map((id) => this.getFileName(id))).toEqual([fileName, fileName]);
          expect(bundle[fileName]).toMatchObject({
            type: 'chunk',
            name: 'group',
            exports: ['a', 'b'],
          });
          const mainFileName = options.format === 'cjs' ? 'chunks/fixed-a2.js' : 'main.js';
          expect(bundle[mainFileName]).toMatchObject({ type: 'chunk', name: 'main' });
          const suffix = options.format === 'cjs' ? 3 : 2;
          const assetFileName = `chunks/fixed-a${suffix}.js`;
          expect(this.getFileName(assetReferenceId)).toBe(assetFileName);
          expect(bundle[assetFileName]).toMatchObject({ type: 'asset', source: 'asset' });
          expect(bundle[this.getFileName(mapAssetReferenceId)]).toMatchObject({
            type: 'asset',
            source: 'map asset',
          });
          expect(bundle[this.getFileName(directoryAssetReferenceId)]).toMatchObject({
            type: 'asset',
            source: 'directory asset',
          });
          const sourcemap = bundle[mapFileName];
          assert(sourcemap?.type === 'asset');
          expect(JSON.parse(sourcemap.source.toString())).toMatchObject({ version: 3 });
          const lateAssetId = this.emitFile({
            type: 'asset',
            name: fileName,
            source: 'late asset',
          });
          expect(this.getFileName(lateAssetId)).toBe(`chunks/fixed-a${suffix + 1}.js`);
        },
      },
    ],
  },
});
