import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'emit-asset',
        load(id) {
          if (!id.endsWith('main.js')) return;
          const referenceId = this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            source: fs.readFileSync(path.join(import.meta.dirname, 'asset.txt')),
          });
          return `export const url = import.meta.ROLLUP_FILE_URL_${referenceId};`;
        },
      },
      {
        name: 'returns-garbage',
        resolveFileUrl() {
          return `'unterminated`;
        },
      },
    ],
  },
  afterTest: () => {
    throw new Error('expected the build to fail on unparsable resolveFileUrl output');
  },
  catchError: (err) => {
    // The plugin name rides along with the returned code, so the failure that surfaces
    // from the finalizer still names both the hook and the plugin — and is reported as a
    // plugin error rather than an internal "please report this" error.
    expect(String(err)).toContain('resolveFileUrl');
    expect(String(err)).toContain('returns-garbage');
    expect(String(err)).not.toContain('UNHANDLEABLE_ERROR');
  },
});
