import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: (string | undefined)[] = [];

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
          // Only `ROLLDOWN_FILE_URL_<referenceId>_<urlId>` carries a `urlId`. The plain
          // `ROLLDOWN_FILE_URL_<referenceId>` form and the `ROLLUP_FILE_URL_` alias do not
          // (for the alias, a trailing `_<urlId>` would be read as part of the reference id).
          return [
            `export const a = import.meta.ROLLDOWN_FILE_URL_${referenceId}_myUrlId;`,
            `export const b = import.meta.ROLLDOWN_FILE_URL_${referenceId};`,
            `export const c = import.meta.ROLLUP_FILE_URL_${referenceId};`,
            `export const d = import.meta.ROLLDOWN_FILE_URL_${referenceId}_defaultUrlId;`,
          ].join('\n');
        },
      },
      {
        name: 'resolve-file-url',
        resolveFileUrl(args) {
          seen.push(args.urlId);
          if (args.urlId === 'defaultUrlId') return null;
          return JSON.stringify(args.urlId ?? null);
        },
      },
    ],
  },
  afterTest: async (output) => {
    // One hook call per occurrence, in source order.
    // - `ROLLDOWN_FILE_URL_<id>_myUrlId` -> `urlId === 'myUrlId'`
    // - `ROLLDOWN_FILE_URL_<id>`         -> `urlId === undefined`
    // - `ROLLUP_FILE_URL_<id>`           -> `urlId === undefined` (alias never carries a urlId)
    // - `ROLLDOWN_FILE_URL_<id>_defaultUrlId` -> `urlId === 'defaultUrlId'`
    expect(seen).toEqual(['myUrlId', undefined, undefined, 'defaultUrlId']);

    const chunk = output.output.find((o) => o.type === 'chunk')!;
    const asset = output.output.find((o) => o.type === 'asset')!;
    expect(chunk.code).toContain('const a = "myUrlId"');
    expect(chunk.code).toContain('const b = null');
    expect(chunk.code).toContain('const c = null');
    // Declining the urlId-carrying occurrence uses the default rewrite. The urlId
    // suffix is ignored when resolving the emitted asset's relative path.
    expect(chunk.code).toContain(
      `const d = new URL(${JSON.stringify(asset.fileName)}, import.meta.url).href`,
    );
  },
});
