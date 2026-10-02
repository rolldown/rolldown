// Native-MagicString ownership on the threadless-WASI flavor, through the BUILT
// `@rolldown/browser` node entry (the same bundle the browser entry wires up,
// loading `rolldown-binding.wasm32-wasip1`). That flavor never gets its GC
// finalizers run, so a `meta.magicString` box -- the source text plus a UTF-16
// mapping table, together about nine times the source bytes -- stays resident
// forever unless the hook wrapper releases it when the invocation settles.
//
// The fixture tests `fixtures/plugin/native-magic-string-eager-release` and
// `fixtures/plugin/render-chunk/native-magic-string-interleave` cover the
// `meta.magicString` box, the retained-meta mint and the concurrent renderChunk
// case on whichever flavor the suite runs against; this one pins a box the
// plugin built itself, which no fixture covers.
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
    'the transform wrapper releases a MagicString the hook built and returned',
    async () => {
      const { build, RolldownMagicString: MagicString } = (await import(
        distEntryPath
      )) as typeof browserEntryTypes;
      const { getRuntimeSupport } = (await import(
        distExperimentalPath
      )) as typeof browserExperimentalTypes;
      // Guard the premise: on a lazy flavor the wrapper drops nothing.
      expect(getRuntimeSupport().threadlessWasi).toBe(true);

      let ownMagicString: RolldownMagicString | undefined;

      const result = await build({
        input: ENTRY_ID,
        write: false,
        experimental: { nativeMagicString: true },
        output: { format: 'esm', sourcemap: true },
        plugins: [
          virtualEntryPlugin(),
          {
            name: 'own-magic-string',
            transform(code: string, id: string) {
              if (id !== ENTRY_ID) return null;
              // NOT `meta.magicString`: a box the plugin minted itself. The
              // wrapper's settle-time cleanup only reached the one the getter
              // mints, so this one's mapping table -- the bulk of the ~9x
              // footprint -- stayed resident for the life of the isolate even
              // though `sendMagicString` had already made the object unusable.
              ownMagicString = new MagicString(code);
              ownMagicString.append('\nconsole.log("own-transformed");');
              return { code: ownMagicString, map: null };
            },
          },
        ],
      });

      // The hook still did its job and the map still came through the channel.
      expect(result.output[0].code).toContain('own-transformed');
      expect(result.output[0].map?.mappings).toBeTruthy();

      expect(ownMagicString).toBeDefined();
      // Already released by the wrapper: nothing is left for this call.
      expect(ownMagicString!.dropInner()).toEqual({
        freed: false,
        reason: 'Memory has already been freed',
      });
      expect(() => ownMagicString!.toString()).toThrow(/no longer usable/);
    },
    180_000,
  );
});
