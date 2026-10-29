import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];

// With hashed chunk filenames, `resolveFileUrl` runs before hashes are computed:
// `chunkId` is a preliminary name carrying a hash placeholder. Code returned by
// the hook that embeds it must still land on the final hashed filename, because
// rendering substitutes the placeholder inside the returned code too — even for
// the chunk's own, self-referential name. The reference lives in a dynamically
// imported module, so the chunk seeing the placeholder is a `chunkFileNames`
// chunk, not the entry.
export default defineTest({
  config: {
    output: {
      entryFileNames: '[name]-[hash].js',
      // Chunks and assets land in different directories, so `relativePath`
      // (chunk → asset) differs from `fileName` (out dir root → asset).
      chunkFileNames: 'chunks/[name]-[hash].js',
      assetFileNames: 'static/[name]-[hash][extname]',
    },
    plugins: [
      {
        name: 'emit-asset',
        load(id) {
          if (!id.endsWith('lazy.js')) return;
          const referenceId = this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            source: fs.readFileSync(path.join(import.meta.dirname, 'asset.txt')),
          });
          return `export const url = import.meta.ROLLUP_FILE_URL_${referenceId};`;
        },
      },
      {
        name: 'resolve-file-url',
        resolveFileUrl(args) {
          seen.push({ ...args });
          return `'chunk:' + ${JSON.stringify(args.chunkId)} + '|' + ${JSON.stringify(args.relativePath)}`;
        },
      },
    ],
  },
  afterTest: async (output) => {
    expect(seen).toHaveLength(1);
    const args = seen[0];
    expect(args.chunkId).toMatch(/^chunks\/lazy-.+\.js$/);
    expect(args.moduleId.replace(/\\/g, '/')).toContain(
      'resolve-file-url/hashed-filenames/lazy.js',
    );
    // Emitted assets are content-hashed when emitted, so `fileName` is already
    // final at hook time; only chunk names defer their hash behind a placeholder.
    expect(args.fileName).toMatch(/^static\/asset-\w+\.txt$/);
    // The referencing chunk lives in `chunks/`, so the path to the asset climbs
    // out of it — `relativePath` is not just `fileName`.
    expect(args.relativePath).toBe(`../${args.fileName}`);

    const lazy = output.output.find((o) => o.fileName.startsWith('chunks/lazy-'))!;
    expect(lazy.fileName).toMatch(/^chunks\/lazy-[\w-]+\.js$/);
    const asset = output.output.find((o) => o.type === 'asset')!;
    expect(asset.fileName).toBe(args.fileName);

    // The hook ran before hashing: the chunk name it saw was a placeholder,
    // not the final hashed name.
    expect(args.chunkId).not.toBe(lazy.fileName);

    // The placeholder embedded in the plugin-returned code was substituted with
    // the real hash, landing exactly on the chunk's own final filename.
    const code = lazy.type === 'chunk' ? lazy.code : '';
    expect(code).toContain(`"chunk:${lazy.fileName}|../${asset.fileName}"`);
    expect(code).not.toContain('!~{');

    // The entry references the same final name for the lazy chunk.
    const main = output.output.find((o) => o.fileName.startsWith('main-'))!;
    expect(main.type === 'chunk' && main.code).toContain(`./${lazy.fileName}`);
  },
});
