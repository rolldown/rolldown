import { defineTest } from 'rolldown-tests'
import { freeExternalMemory } from 'rolldown/experimental'
import { expect } from 'vitest'

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
          })
        },
      },
    ],
  },
  afterTest(output) {
    // This test primarily ensures TypeScript type correctness in main.ts
    // The actual runtime behavior is tested in object-properties test
    // Here we just verify the API works at runtime as well

    // Test 1: Can call freeExternalMemory on RolldownOutput
    const result1 = freeExternalMemory(output)
    expect(typeof result1).toBe('boolean')

    // Test 2: Can call freeExternalMemory on OutputChunk
    const chunk = output.output.find((item) => item.type === 'chunk')
    expect(chunk).toBeDefined()
    if (chunk) {
      const result2 = freeExternalMemory(chunk)
      expect(typeof result2).toBe('boolean')
    }

    // Test 3: Can call freeExternalMemory on OutputAsset
    const asset = output.output.find((item) => item.type === 'asset')
    expect(asset).toBeDefined()
    if (asset) {
      const result3 = freeExternalMemory(asset)
      expect(typeof result3).toBe('boolean')
    }
  },
})
