// - `unique symbol` can't be used in computed properties with `isolatedDeclarations: true`
// - https://github.com/microsoft/typescript/issues/61892
const symbolForExternalMemoryHandle =
  '__rolldown_external_memory_handle__' as const;

/**
 * Interface for objects that hold external memory that can be explicitly freed.
 */
export interface ExternalMemoryHandle {
  /**
   * Frees the external memory held by this object.
   * @param keepDataAlive - If true, evaluates all lazy fields before freeing memory.
   *   This will take time but prevents errors when accessing properties after freeing.
   * @returns `true` if memory was successfully freed, `false` if it was already freed.
   * @internal
   */
  [symbolForExternalMemoryHandle]: (keepDataAlive?: boolean) => boolean;
}

/**
 * Frees the external memory held by the given handle.
 *
 * This is useful when you want to manually release memory held by Rust objects
 * (like `OutputChunk` or `OutputAsset`) before they are garbage collected.
 *
 * @param handle - The object with external memory to free
 * @param keepDataAlive - If true, evaluates all lazy fields before freeing memory (default: false).
 *   This will take time to copy data from Rust to JavaScript, but prevents errors
 *   when accessing properties after the memory is freed.
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
 * // Manually free the memory (fast, but accessing properties after will throw)
 * const freed = freeExternalMemory(chunk); // true
 * const alreadyFreed = freeExternalMemory(chunk); // false
 *
 * // Keep data alive before freeing (slower, but data remains accessible)
 * freeExternalMemory(chunk, true); // Evaluates all lazy fields first
 * console.log(chunk.code); // OK - data was copied to JavaScript before freeing
 *
 * // Without keepDataAlive, accessing chunk properties after freeing will throw an error
 * ```
 */
export function freeExternalMemory(
  handle: ExternalMemoryHandle,
  keepDataAlive = false,
): boolean {
  return handle[symbolForExternalMemoryHandle](keepDataAlive);
}
