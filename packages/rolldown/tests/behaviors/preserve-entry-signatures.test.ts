import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';
import { getOutputChunk } from '../src/utils';

function virtualPlugin(modules: Record<string, string>): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      return id in modules ? id : undefined;
    },
    load(id) {
      return modules[id];
    },
  };
}

describe.each(['external', 'barrel'])('external star via %s', (starSource) => {
  test.each(['esm', 'cjs'])('preserves exports-only entries sharing %s bindings', async (kind) => {
    const bundle = await rolldown({
      input: 'main',
      preserveEntrySignatures: 'exports-only',
      external: ['external'],
      plugins: [
        virtualPlugin({
          main: `export * from '${starSource}';
            import value from 'shared';
            globalThis.value = value;
            globalThis.load = () => import('dynamic');`,
          barrel: 'export * from "external";',
          shared:
            kind === 'esm' ? 'export default { value: 42 };' : 'module.exports = { value: 42 };',
          dynamic: 'export { default } from "shared";',
        }),
      ],
    });
    try {
      for (const format of ['es', 'cjs'] as const) {
        const chunks = getOutputChunk(await bundle.generate({ format }));
        const entry = chunks.find((chunk) => chunk.facadeModuleId === 'main')!;
        expect(entry.exports).toEqual([]);
        expect(entry.imports).toContain('external');
      }
    } finally {
      await bundle.close();
    }
  });
});

test.each(['es', 'cjs'] as const)(
  'strict emitted entries preserve exports and file references in %s output',
  async (format) => {
    let referenceId: string;
    let resolvedFileName: string | undefined;
    const bundle = await rolldown({
      input: 'app.js',
      preserveEntrySignatures: false,
      plugins: [
        {
          name: 'emit-entry',
          buildStart() {
            referenceId = this.emitFile({
              type: 'chunk',
              id: 'entry.js',
              fileName: 'public/handler.js',
              preserveSignature: 'strict',
            });
          },
          load(id) {
            if (id === 'app.js') {
              return `console.log(import.meta.ROLLUP_FILE_URL_${referenceId});`;
            }
          },
          resolveFileUrl({ referenceId: resolved, fileName }) {
            expect(resolved).toBe(referenceId);
            expect(this.getFileName(resolved)).toBe(fileName);
            resolvedFileName = fileName;
            return JSON.stringify(fileName);
          },
        },
        virtualPlugin({
          'app.js': '',
          'entry.js': `import shared from 'shared.cjs';
            export const POST = async () => shared === (await import('dynamic.cjs')).default;`,
          'shared.cjs': 'module.exports = { tag: "shared" };',
          'dynamic.cjs': 'module.exports = require("shared.cjs");',
        }),
      ],
    });
    try {
      for (const minifyInternalExports of [false, true]) {
        const chunks = getOutputChunk(
          await bundle.generate({
            format,
            minifyInternalExports,
            entryFileNames: '[name]-[hash].js',
            chunkFileNames: 'chunks/[name]-[hash].js',
          }),
        );
        const entry = chunks.find((chunk) => chunk.fileName === 'public/handler.js')!;
        expect(entry.isEntry).toBe(true);
        expect(entry.facadeModuleId).toBe('entry.js');
        expect(entry.exports).toEqual(['POST']);
        expect(entry.moduleIds).toEqual([]);
        expect(resolvedFileName).toBe(entry.fileName);
        const implementation = chunks.find((chunk) => chunk.moduleIds.includes('entry.js'))!;
        expect(implementation.isEntry).toBe(false);
        expect(entry.imports).toContain(implementation.fileName);
      }
    } finally {
      await bundle.close();
    }
  },
);

test('a strict entry keeps a wrapper whose uses are all in the same chunk', async () => {
  const bundle = await rolldown({
    input: 'main.js',
    preserveEntrySignatures: 'strict',
    experimental: {
      chunkOptimization: { mergeCommonChunks: true, avoidRedundantChunkLoads: false },
    },
    plugins: [
      virtualPlugin({
        'main.js': "export { POST } from 'private.js';",
        'private.js': `import shared from 'shared.cjs';
        export const POST = async () => shared === (await import('shared.cjs')).default;`,
        'shared.cjs': 'module.exports = { tag: "shared" };',
      }),
    ],
  });
  try {
    const chunks = getOutputChunk(await bundle.generate({}));
    expect(chunks).toHaveLength(1);
    expect(chunks[0].exports).toEqual(['POST']);
    const main = await import(`data:text/javascript,${encodeURIComponent(chunks[0].code)}`);
    expect(Object.keys(main)).toEqual(['POST']);
    expect(await main.POST()).toBe(true);
  } finally {
    await bundle.close();
  }
});

test.each(['strict', 'exports-only'] as const)(
  'preserves a shim-only entry with %s',
  async (preserveEntrySignatures) => {
    const bundle = await rolldown({
      input: ['main.js', 'entry.js'],
      preserveEntrySignatures,
      shimMissingExports: true,
      plugins: [
        virtualPlugin({
          'main.js': `import { missing, unused } from 'entry.js';
            export { missing };
            export const load = () => import('dynamic.cjs');`,
          'entry.js': `import shared from 'shared.cjs'; globalThis.shared = shared; export {};`,
          'shared.cjs': 'module.exports = { value: 42 };',
          'dynamic.cjs': 'module.exports = require("shared.cjs");',
        }),
      ],
    });
    try {
      for (const format of ['es', 'cjs'] as const) {
        for (const minifyInternalExports of [false, true]) {
          const chunks = getOutputChunk(await bundle.generate({ format, minifyInternalExports }));
          const entry = chunks.find((chunk) => chunk.fileName === 'entry.js')!;
          expect(entry.exports).toEqual(['missing', 'unused']);
        }
      }
    } finally {
      await bundle.close();
    }
  },
);
