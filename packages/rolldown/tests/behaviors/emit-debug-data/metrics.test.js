import { rolldown } from 'rolldown';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from 'vitest';

test.each(['generate', 'write'])(
  'chunk metrics follow generateBundle changes in %s',
  async (method) => {
    const cwd = mkdtempSync(join(tmpdir(), 'rolldown-chunk-metrics-'));
    const sources = {
      main: 'import { value } from "shared"; console.log(value); import("removed");',
      shared: 'export const value = 1;',
      removed: 'console.log("optional"); export const optional = 2;',
    };
    let bundle;
    try {
      bundle = await rolldown({
        cwd,
        input: { main: 'main', shared: 'shared' },
        devtools: { mode: 'report', reportDir: 'metrics', reportTopN: 1 },
        plugins: [
          {
            name: 'change-output',
            resolveId(id) {
              return id in sources ? id : null;
            },
            load(id) {
              return sources[id];
            },
            generateBundle(_, output) {
              const main = output['main.js'];
              const removed = main.dynamicImports[0];
              expect(output[removed]?.type).toBe('chunk');
              delete output[removed];
              main.fileName = 'renamed.js';
              main.code = 'import "./renamed-shared.js";';
              main.imports = ['renamed-shared.js'];
              main.dynamicImports = [];
              const shared = output['shared.js'];
              shared.fileName = 'renamed-shared.js';
            },
          },
        ],
      });
      const { output } = await bundle[method]({
        dir: 'dist',
        entryFileNames: '[name].js',
        chunkFileNames: '[name].js',
      });
      await bundle.close();
      const graph = JSON.parse(readFileSync(join(cwd, 'metrics/chunk-graph.json'), 'utf8'));
      expect(graph.chunks).toEqual(
        output
          .filter((chunk) => chunk.type === 'chunk')
          .map((chunk) => ({
            fileName: chunk.fileName,
            name: chunk.name,
            bytes: Buffer.byteLength(chunk.code),
            isEntry: chunk.isEntry || chunk.isDynamicEntry,
            facadeModuleId: chunk.facadeModuleId,
            imports: chunk.imports,
            dynamicImports: chunk.dynamicImports,
          }))
          .sort((a, b) => a.fileName.localeCompare(b.fileName)),
      );
      expect(graph.chunks.map((chunk) => chunk.fileName)).toEqual([
        'renamed-shared.js',
        'renamed.js',
      ]);
      expect(graph.chunks[1].imports).toEqual(['renamed-shared.js']);
      expect(graph.chunks[1].dynamicImports).toEqual([]);
    } finally {
      await bundle?.close();
      rmSync(cwd, { recursive: true, force: true });
    }
  },
);
