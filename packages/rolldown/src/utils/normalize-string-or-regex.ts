import type { BindingStringOrRegex } from '../binding';

/*
 * Normalize single or multiple string or regex patterns to an array of BindingStringOrRegex
 * convert a type that is dx friendly to a type that is friendly for binding usage
 */
export function normalizedStringOrRegex(
  pattern?: Array<string | RegExp> | (string | RegExp),
): BindingStringOrRegex[] | undefined {
  if (!pattern) {
    return undefined;
  }
  if (!Array.isArray(pattern)) {
    pattern = [pattern];
  }
  return pattern;
}
