import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// https://github.com/rolldown/rolldown/issues/9994
// `emitFile({ type: 'chunk' })` must reject a path-fragment `name`/`fileName`
// synchronously at emit time, like the asset case in `emit-file-invalid-name`.
const INVALID_NAME =
  'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          // Path-fragment `name` values are rejected synchronously.
          for (const name of ['./relative', '../parent', '/absolute']) {
            expect(() => {
              this.emitFile({ type: 'chunk', id: './main.js', name });
            }).toThrow(INVALID_NAME);
          }

          // Path-fragment `fileName` values are rejected too.
          for (const fileName of ['../parent.js', '/absolute.js']) {
            expect(() => {
              this.emitFile({ type: 'chunk', id: './main.js', fileName });
            }).toThrow(INVALID_NAME);
          }

          // Windows absolute path (only meaningful on Windows).
          if (process.platform === 'win32') {
            expect(() => {
              this.emitFile({ type: 'chunk', id: './main.js', name: 'C:/windows' });
            }).toThrow(INVALID_NAME);
          }

          // Valid names are still accepted: a bare name, and a subdirectory
          // `fileName` (subdirectories are not path fragments).
          this.emitFile({ type: 'chunk', id: './main.js', name: 'valid-name' });
          this.emitFile({ type: 'chunk', id: './main.js', fileName: 'nested/dir/valid.js' });
        },
      },
    ],
  },
});
