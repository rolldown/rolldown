// Final-output ownership on the threadless-WASI flavor, through the BUILT
// `@rolldown/browser` node entry (the same bundle the browser entry wires up,
// loading `rolldown-binding.wasm32-wasip1`). That flavor never gets its GC
// finalizers run, so the `BindingOutputChunk`/`BindingOutputAsset` boxes behind
// a build result stay resident forever unless something releases them.
// `RolldownOutput.output` releases them from its lazy getter -- which never
// runs for a caller that ignores the returned output (the write-and-forget
// pattern), so `RolldownBuild#build` must materialize the result itself on
// this flavor. This test pins that: build through the standard API, never read
// `result.output`, then assert the native boxes are already released.
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';
// @ts-ignore Type-only view of the browser entry; the dist bundle is imported at runtime.
import type * as browserEntryTypes from '../src/index';
// @ts-ignore Type-only view of the experimental entry.
import type * as browserExperimentalTypes from '../src/experimental-index';
// @ts-ignore Type-only view of the binding output boxes.
import type { BindingOutputs } from '../src/binding.cjs';

const distDir = new URL('../../browser/dist/', import.meta.url);
const distEntryPath = fileURLToPath(new URL('index.mjs', distDir));
const distExperimentalPath = fileURLToPath(new URL('experimental-index.mjs', distDir));
const distWasmPath = fileURLToPath(new URL('rolldown-binding.wasm32-wasip1.wasm', distDir));

const distTest = test.runIf(
  existsSync(distEntryPath) && existsSync(distExperimentalPath) && existsSync(distWasmPath),
);

const ENTRY_ID = 'virt:entry.js';
const ENTRY_CODE = 'export const answer = 42;\nconsole.log(answer);\n';
const ASSET_NAME = 'note.txt';
const ASSET_SOURCE = 'ignored-output asset payload\n';

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

// The final result's binding boxes, reached past the public surface on
// purpose: the point is to observe their release WITHOUT touching
// `result.output`, whose lazy getter releases them as a side effect.
function rawBindingOutputs(result: unknown): BindingOutputs {
  const boxes = (result as { bindingOutputs?: BindingOutputs }).bindingOutputs;
  if (!boxes) {
    throw new Error(
      'RolldownOutputImpl no longer stores its BindingOutputs on `bindingOutputs`; ' +
        'update this test to reach the raw boxes however the implementation now holds them.',
    );
  }
  return boxes;
}

describe('ignored build output on the threadless-WASI dist', () => {
  distTest(
    'write() releases the native output boxes even when the result is never read',
    async () => {
      const { rolldown } = (await import(distEntryPath)) as typeof browserEntryTypes;
      const { getRuntimeSupport } = (await import(
        distExperimentalPath
      )) as typeof browserExperimentalTypes;
      // Guard the premise: without this the assertions below would pass
      // vacuously against a lazy flavor.
      expect(getRuntimeSupport().threadlessWasi).toBe(true);

      const outDir = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-ignored-output-'));
      try {
        const bundle = await rolldown({
          input: ENTRY_ID,
          plugins: [
            virtualEntryPlugin(),
            {
              name: 'emit-ignored-asset',
              generateBundle() {
                this.emitFile({ type: 'asset', fileName: ASSET_NAME, source: ASSET_SOURCE });
              },
            },
          ],
        });
        const result = await bundle.write({ format: 'esm', dir: outDir });
        // The write-and-forget pattern: `result.output` is deliberately not
        // read before the raw boxes are inspected. close() is also no help --
        // it never owned these boxes -- so the release below must have come
        // from the build path itself.
        await bundle.close();

        const raw = rawBindingOutputs(result);
        expect(raw.chunks.length).toBeGreaterThan(0);
        expect(raw.assets.length).toBeGreaterThan(0);
        for (const [label, box] of [
          ...raw.chunks.map((chunk, i) => [`chunk[${i}]`, chunk] as const),
          ...raw.assets.map((asset, i) => [`asset[${i}]`, asset] as const),
        ]) {
          // Already released by the build path: nothing is left for this call.
          expect(box.dropInner(), label).toEqual({
            freed: false,
            reason: 'Memory has already been freed',
          });
          // A released box refuses reads instead of resolving through freed
          // native memory.
          expect(() => box.getFileName(), label).toThrow(/Memory has been freed/);
        }

        // The materialized JavaScript copy is what the caller reads if it ever
        // does come back for the result: still fully usable after the native
        // release and the close.
        const chunk = result.output.find((item) => item.type === 'chunk');
        expect(chunk?.code).toContain('answer = 42');
        const asset = result.output.find((item) => item.type === 'asset');
        expect(asset && String(asset.source)).toBe(ASSET_SOURCE);
        // And the write itself really happened.
        expect(chunk).toBeDefined();
        const written = await readFile(nodePath.join(outDir, chunk!.fileName), 'utf8');
        expect(written).toContain('answer = 42');
      } finally {
        await rm(outDir, { recursive: true, force: true });
      }
    },
    180_000,
  );
});
