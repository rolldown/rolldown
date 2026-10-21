// A threadless WASI host may never run GC finalizers, so the transform wrapper
// must release a MagicString the plugin built itself and returned (the
// `meta.magicString` cases live in the native-magic-string fixtures). Runs
// against the built `@rolldown/browser` node entry (wasm32-wasip1).
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';
import type * as browserEntryTypes from '../src/index';
import type * as browserExperimentalTypes from '../src/experimental-index';
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
              // NOT `meta.magicString`: `sendMagicString` leaves this box's
              // mapping table behind, so the wrapper must drop it too.
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
