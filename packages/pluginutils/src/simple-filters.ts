/**
 * Constructs a RegExp that matches the exact string specified.
 *
 * This is useful for plugin hook filters.
 *
 * @param str the string to match.
 * @param flags flags for the RegExp.
 */
export function exactRegex(str: string, flags?: string): RegExp {
  return new RegExp(`^${escapeRegex(str)}$`, flags);
}

/**
 * Constructs a RegExp that matches a value that has the specified prefix.
 *
 * This is useful for plugin hook filters.
 *
 * @param str the string to match.
 * @param flags flags for the RegExp.
 */
export function prefixRegex(str: string, flags?: string): RegExp {
  return new RegExp(`^${escapeRegex(str)}`, flags);
}

const escapeRegexRE = /[-/\\^$*+?.()|[\]{}]/g;
function escapeRegex(str: string): string {
  return str.replace(escapeRegexRE, '\\$&');
}
