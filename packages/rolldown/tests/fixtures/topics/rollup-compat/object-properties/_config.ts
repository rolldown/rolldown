import { defineTest } from 'rolldown-tests'
import { getOutputAsset, getOutputChunk } from 'rolldown-tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-emit-asset',
        buildStart() {
          // Emit an asset to ensure we have both chunks and assets to test
          this.emitFile({
            type: 'asset',
            name: 'test-asset.txt',
            source: 'test content',
          })
        },
      },
    ],
  },
  afterTest(output) {
    // Test 1: OutputChunk instances should have non-enumerable __rolldown_external_memory_handle__
    const chunks = getOutputChunk(output)
    expect(chunks.length).toBeGreaterThan(0)

    for (const chunk of chunks) {
      // Verify the method exists and is callable
      expect(typeof chunk.__rolldown_external_memory_handle__).toBe('function')

      // Verify it's not enumerable (doesn't appear in Object.keys)
      expect(Object.keys(chunk)).not.toContain('__rolldown_external_memory_handle__')

      // Verify the property descriptor shows enumerable: false
      // The method is on the prototype, so we need to check the prototype chain
      const descriptor = Object.getOwnPropertyDescriptor(
        Object.getPrototypeOf(chunk),
        '__rolldown_external_memory_handle__',
      )
      expect(descriptor).toBeDefined()
      expect(descriptor?.enumerable).toBe(false)
    }

    // Test 2: OutputAsset instances should have non-enumerable __rolldown_external_memory_handle__
    const assets = getOutputAsset(output)
    expect(assets.length).toBeGreaterThan(0)

    for (const asset of assets) {
      // Verify the method exists and is callable
      expect(typeof asset.__rolldown_external_memory_handle__).toBe('function')

      // Verify it's not enumerable (doesn't appear in Object.keys)
      expect(Object.keys(asset)).not.toContain('__rolldown_external_memory_handle__')

      // Verify the property descriptor shows enumerable: false
      // The method is on the prototype, so we need to check the prototype chain
      const descriptor = Object.getOwnPropertyDescriptor(
        Object.getPrototypeOf(asset),
        '__rolldown_external_memory_handle__',
      )
      expect(descriptor).toBeDefined()
      expect(descriptor?.enumerable).toBe(false)
    }

    // Test 3: RolldownOutput instance should have non-enumerable __rolldown_external_memory_handle__
    // Verify the method exists and is callable
    expect(typeof output.__rolldown_external_memory_handle__).toBe('function')

    // Verify it's not enumerable (doesn't appear in Object.keys)
    expect(Object.keys(output)).not.toContain('__rolldown_external_memory_handle__')

    // Verify the property descriptor shows enumerable: false
    // The method is on the prototype, so we need to check the prototype chain
    const descriptor = Object.getOwnPropertyDescriptor(
      Object.getPrototypeOf(output),
      '__rolldown_external_memory_handle__',
    )
    expect(descriptor).toBeDefined()
    expect(descriptor?.enumerable).toBe(false)
  },
})
