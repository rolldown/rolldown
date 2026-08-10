// Native-MagicString ownership on the threadless-WASI flavor, through the BUILT
// `@rolldown/browser` node entry (the same bundle the browser entry wires up,
// loading `rolldown-binding.wasm32-wasip1`). That flavor never gets its GC
// finalizers run, so a `meta.magicString` box -- the source text plus a UTF-16
// mapping table, together about nine times the source bytes -- stays resident
// forever unless the hook wrapper releases it when the invocation settles.
//
// The fixture test `fixtures/plugin/native-magic-string-eager-release` covers
// the same contract on whichever flavor the suite runs against; this one pins
// the threadless half without needing the whole suite switched over to it.
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';
// @ts-ignore Type-only view of the browser entry; the dist bundle is imported at runtime.
import type * as browserEntryTypes from '../src/index';
// @ts-ignore Type-only view of the experimental entry.
import type * as browserExperimentalTypes from '../src/experimental-index';
// @ts-ignore Type-only view of the magic-string wrapper.
import type { RolldownMagicString } from '../src/binding-magic-string';

const distDir = new URL('../../browser/dist/', import.meta.url);
const distEntryPath = fileURLToPath(new URL('index.mjs', distDir));
const distExperimentalPath = fileURLToPath(new URL('experimental-index.mjs', distDir));
const distWasmPath = fileURLToPath(new URL('rolldown-binding.wasm32-wasip1.wasm', distDir));

const distTest = test.runIf(
  existsSync(distEntryPath) && existsSync(distExperimentalPath) && existsSync(distWasmPath),
);

const ENTRY_ID = 'virt:entry.js';
const ENTRY_CODE = 'export const answer = 42;\nconsole.log(answer);\n';

function virtualEntryPlugin(): {
  name: string;
  resolveId: (id: string) => string | undefined;
  load: (id: string) => string | undefined;
} {
  return {
    name: 'virtual-entry',
    resolveId: (id: string) => (id === ENTRY_ID ? id : undefined),
    load: (id: string) => (id === ENTRY_ID ? ENTRY_CODE : undefined),
  };
}

describe('native MagicString ownership on the threadless-WASI dist', () => {
  distTest(
    'the transform and renderChunk wrappers release the magicString they minted',
    async () => {
      const { build } = (await import(distEntryPath)) as typeof browserEntryTypes;
      const { getRuntimeSupport } = (await import(
        distExperimentalPath
      )) as typeof browserExperimentalTypes;
      // Guard the premise: without this the assertions below would pass
      // vacuously against a lazy flavor.
      expect(getRuntimeSupport().threadlessWasi).toBe(true);

      let transformMagicString: RolldownMagicString | undefined;
      let renderChunkMagicString: RolldownMagicString | undefined;

      const result = await build({
        input: ENTRY_ID,
        write: false,
        experimental: { nativeMagicString: true },
        output: { format: 'esm', sourcemap: true },
        plugins: [
          virtualEntryPlugin(),
          {
            name: 'magic-string-probe',
            transform(_code, _id, meta) {
              if (!meta?.magicString) return null;
              transformMagicString = meta.magicString;
              transformMagicString.append('\nconsole.log("transformed");');
              // Returning it hands the string itself to `sendMagicString`,
              // which leaves the mapping table -- the bulk of the footprint --
              // behind; `map: null` says the map went out-of-band.
              return { code: transformMagicString, map: null };
            },
            renderChunk(_code, _chunk, _options, meta) {
              if (!meta.magicString) return null;
              renderChunkMagicString = meta.magicString;
              renderChunkMagicString.append('\n// rendered');
              return renderChunkMagicString;
            },
          },
        ],
      });

      // Both hooks still did their job.
      const code = result.output[0].code;
      expect(code).toContain('transformed');
      expect(code).toContain('// rendered');

      for (const [label, box] of [
        ['transform', transformMagicString],
        ['renderChunk', renderChunkMagicString],
      ] as const) {
        expect(box, label).toBeDefined();
        // Already released by the hook wrapper: nothing is left for this call.
        expect(box!.dropInner(), label).toEqual({
          freed: false,
          reason: 'Memory has already been freed',
        });
        // A repeated drop reports, it never crashes.
        expect(box!.dropInner(), label).toEqual({
          freed: false,
          reason: 'Memory has already been freed',
        });
        // A released instance refuses reads instead of handing back the empty
        // `MagicString` left behind.
        expect(() => box!.toString(), label).toThrow(/no longer usable/);
      }
    },
    180_000,
  );
});
