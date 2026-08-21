import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Test that emitFile rejects invalid asset names (absolute/relative paths)
// This matches Rollup's behavior
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          // Test relative path starting with "./"
          expect(() => {
            this.emitFile({
              type: 'asset',
              name: './relative.txt',
              source: 'content',
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
          );

          // Test relative path starting with "../"
          expect(() => {
            this.emitFile({
              type: 'asset',
              name: '../parent.txt',
              source: 'content',
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
          );

          // Test absolute Unix path
          expect(() => {
            this.emitFile({
              type: 'asset',
              name: '/absolute.txt',
              source: 'content',
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
          );

          // Test Windows absolute path (only on Windows)
          if (process.platform === 'win32') {
            expect(() => {
              this.emitFile({
                type: 'asset',
                name: 'C:/windows.txt',
                source: 'content',
              });
            }).toThrow(
              'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
            );
          }

          // Chunk names are validated the same way
          expect(() => {
            this.emitFile({
              type: 'chunk',
              id: './main.js',
              name: '../node_modules/some-lib/entry',
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths, received "../node_modules/some-lib/entry".',
          );

          // Chunk `fileName` is validated the same way as `name`
          expect(() => {
            this.emitFile({
              type: 'chunk',
              id: './main.js',
              fileName: '../out/entry.js',
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths, received "../out/entry.js".',
          );

          // Absolute chunk name
          expect(() => {
            this.emitFile({
              type: 'chunk',
              id: './main.js',
              name: '/abs-entry',
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths, received "/abs-entry".',
          );

          // Windows drive-letter names are rejected regardless of the host OS
          expect(() => {
            this.emitFile({
              type: 'asset',
              name: 'F:\\win.txt',
              source: 'content',
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths, received "F:\\win.txt".',
          );

          // Prebuilt chunk file names are validated too, with a prebuilt-specific message
          expect(() => {
            this.emitFile({
              type: 'prebuilt-chunk',
              fileName: '../prebuilt.js',
              code: 'export default 1;',
            });
          }).toThrow(
            'The "fileName" property of emitted prebuilt chunks must be strings that are neither absolute nor relative paths, received "../prebuilt.js".',
          );

          // A missing/non-string prebuilt `fileName` is rejected by the same check, so it
          // never reaches N-API (which would report `Missing field \`fileName\`` with a
          // napi Status as pluginCode). Mirrors Rollup's
          // test/function/samples/emit-file/invalid-prebuilt-chunk-filename.
          expect(() => {
            this.emitFile({
              type: 'prebuilt-chunk',
              code: 'export default 1;',
            } as any);
          }).toThrow(
            'The "fileName" property of emitted prebuilt chunks must be strings that are neither absolute nor relative paths, received "undefined".',
          );

          // `code` is validated first, as in Rollup's emitPrebuiltChunk. Mirrors
          // test/function/samples/emit-file/invalid-prebuit-chunk-code.
          expect(() => {
            this.emitFile({
              type: 'prebuilt-chunk',
              fileName: 'my-chunk.js',
            } as any);
          }).toThrow(
            'Emitted prebuilt chunks need to have a valid string code, received "undefined".',
          );

          // Subdirectory names are not path fragments
          this.emitFile({
            type: 'asset',
            name: 'sub/dir/asset.txt',
            source: 'content',
          });

          // Emit a valid asset so the build succeeds
          this.emitFile({
            type: 'asset',
            name: 'valid.txt',
            source: 'valid content',
          });
        },
      },
    ],
  },
});
