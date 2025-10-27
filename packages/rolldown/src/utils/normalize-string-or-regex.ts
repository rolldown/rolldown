import type { BindingStringOrRegex } from '../binding';

/*
 * Normalize single or multiple string or regex patterns to an array of BindingStringOrRegex
 * convert a type that is dx friendly to a type that is friendly for binding usage
 */
export function normalizedStringOrRegex<
  T extends Array<BindingStringOrRegex> | ReadonlyArray<BindingStringOrRegex>,
>(
  pattern?: T | BindingStringOrRegex,
): T | undefined {
  if (!pattern) {
    return undefined;
  }
  if (!isReadonlyArray(pattern)) {
    return [pattern] as T;
  }
  return pattern as T;
}

// For https://github.com/microsoft/TypeScript/issues/17002
function isReadonlyArray<T extends unknown[] | readonly unknown[]>(
  input: T | unknown,
): input is T {
  return Array.isArray(input);
}
