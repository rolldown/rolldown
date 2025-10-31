import { defineTest } from 'rolldown-tests';
import { freeExternalMemory } from 'rolldown/experimental';
import { expect } from 'vitest';

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
      const result1 = freeExternalMemory(chunk);
      expect(result1).toHaveProperty('freed');
      expect(result1.freed).toBe(true);

      // Calling again should return freed: false with a reason
      const result1Again = freeExternalMemory(chunk);
      expect(result1Again.freed).toBe(false);
      expect(result1Again.reason).toBeDefined();
      expect(result1Again.reason).toContain('already been freed');

      // After freeing, accessing properties should throw
      expect(() => chunk.name).toThrow();
    }

    // Test 2: Can call freeExternalMemory on OutputAsset
    const asset = output.output.find((item) => item.type === 'asset');
    expect(asset).toBeDefined();
    if (asset) {
      const result2 = freeExternalMemory(asset);
      expect(result2).toHaveProperty('freed');
      expect(result2.freed).toBe(true);
    }

    // Test 3: Can call freeExternalMemory on RolldownOutput (after individual items are freed)
    // This should report that items are already freed
    const result3 = freeExternalMemory(output);
    expect(result3).toHaveProperty('freed');
    expect(typeof result3.freed).toBe('boolean');
    expect(result3.freed).toBe(false);
    expect(result3.reason).toBeDefined();
  },
});
