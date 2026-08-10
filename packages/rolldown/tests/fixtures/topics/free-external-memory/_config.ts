import { isThreadlessWasi } from '@tests/runtime-flavor';
import { defineTest } from 'rolldown-tests';
import { freeExternalMemory } from 'rolldown/experimental';
import { expect } from 'vitest';

// `freeExternalMemory()` has one contract per flavor:
//   native / threaded WASI  fields stay lazy, so the first call hands back the
//                           payload -> { freed: true } and later reads throw.
//   threadless WASI         `transformToRollupOutput()` already materialized
//                           every field and called `dropInner()` (see
//                           src/utils/threadless-free.ts), so nothing is left to
//                           release -> { freed: false, reason: '...already been
//                           freed' } and reads still work off the JS copy.
// Asserting both instead of skipping WASI is what proves the eager-free path
// dropped the payload WITHOUT losing the output.
function expectFirstFree(status: { freed: boolean; reason?: string }): void {
  expect(status).toHaveProperty('freed');
  if (isThreadlessWasi) {
    expect(status.freed).toBe(false);
    expect(status.reason).toContain('already been freed');
  } else {
    expect(status.freed).toBe(true);
  }
}

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-emit-asset',
        buildStart() {
          // Emit an asset to ensure we have both chunks and assets for type testing
          this.emitFile({
            type: 'asset',
            name: 'test.txt',
            source: 'test content for type checking',
          });
        },
      },
    ],
  },
  afterTest(output) {
    // This test primarily ensures TypeScript type correctness in main.ts
    // The actual runtime behavior is tested in object-properties test
    // Here we just verify the API works at runtime as well

    // Test 1: Can call freeExternalMemory on OutputChunk
    const chunk = output.output.find((item) => item.type === 'chunk');
    expect(chunk).toBeDefined();
    if (chunk) {
      expectFirstFree(freeExternalMemory(chunk));

      // Calling again should return freed: false with a reason
      const result1Again = freeExternalMemory(chunk);
      expect(result1Again.freed).toBe(false);
      expect(result1Again.reason).toBeDefined();
      expect(result1Again.reason).toContain('already been freed');

      if (isThreadlessWasi) {
        // The eager path copied every field into the wrapper before dropping:
        // payload gone, data intact.
        expect(chunk.name).toBe('main');
        expect(typeof chunk.code).toBe('string');
      } else {
        // After freeing, accessing properties should throw
        expect(() => chunk.name).toThrow();
      }
    }

    // Test 2: Can call freeExternalMemory on OutputAsset
    const asset = output.output.find((item) => item.type === 'asset');
    expect(asset).toBeDefined();
    if (asset) {
      expectFirstFree(freeExternalMemory(asset));
      if (isThreadlessWasi) {
        expect(asset.source).toBe('test content for type checking');
      }
    }

    // Test 3: Can call freeExternalMemory on RolldownOutput (after individual items are freed)
    // This should report that items are already freed, on every flavor --
    // whether this test freed the payload or the eager path did.
    const result3 = freeExternalMemory(output);
    expect(result3).toHaveProperty('freed');
    expect(typeof result3.freed).toBe('boolean');
    expect(result3.freed).toBe(false);
    expect(result3.reason).toBeDefined();
  },
});
