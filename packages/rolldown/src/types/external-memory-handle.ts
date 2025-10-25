// - `unique symbol` can't be used in computed properties with `isolatedDeclarations: true`
// - https://github.com/microsoft/typescript/issues/61892
export const symbolForExternalMemoryHandle =
  '__rolldown_external_memory_handle__' as const;

/**
 * Interface for objects that hold external memory that can be explicitly freed.
 */
export interface ExternalMemoryHandle {
  /**
   * Frees the external memory held by this object.
   * @returns `true` if memory was successfully freed, `false` if it was already freed.
   * @internal
   */
  [symbolForExternalMemoryHandle]: () => boolean;
}

/**
 * Frees the external memory held by the given handle.
 *
 * This is useful when you want to manually release memory held by Rust objects
 * (like `OutputChunk` or `OutputAsset`) before they are garbage collected.
 *
 * @param handle - The object with external memory to free
 * @returns `true` if memory was successfully freed, `false` if it was already freed
 *
 * @example
 * ```typescript
 * import { freeExternalMemory } from 'rolldown/experimental';
 *
 * const output = await bundle.generate();
 * const chunk = output.output[0];
 *
 * // Use the chunk...
 *
 * // Manually free the memory
 * const freed = freeExternalMemory(chunk); // true
 * const alreadyFreed = freeExternalMemory(chunk); // false
 *
 * // Accessing chunk properties after freeing will throw an error
 * ```
 */
export function freeExternalMemory(handle: ExternalMemoryHandle): boolean {
  return handle[symbolForExternalMemoryHandle]();
}
