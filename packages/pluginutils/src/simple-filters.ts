/**
 * Constructs a RegExp that matches the exact string specified.
 *
 * This is useful for plugin hook filters.
 *
 * @param str the string to match.
 * @param flags flags for the RegExp.
 *
 * @example
 * ```ts
 * import { exactRegex } from '@rolldown/pluginutils';
 * const plugin = {
 *   name: 'plugin',
 *   resolveId: {
 *     filter: { id: exactRegex('foo') },
 *     handler(id) {} // will only be called for `foo`
 *   }
 * }
 * ```
 */
export function exactRegex(str: string | string[], flags?: string): RegExp {
  return new RegExp(`^${combineMultipleStrings(str)}$`, flags);
}

/**
 * Constructs a RegExp that matches a value that has the specified prefix.
 *
 * This is useful for plugin hook filters.
 *
 * @param str the string to match.
 * @param flags flags for the RegExp.
 *
 * @example
 * ```ts
 * import { prefixRegex } from '@rolldown/pluginutils';
 * const plugin = {
 *   name: 'plugin',
 *   resolveId: {
 *     filter: { id: prefixRegex('foo') },
 *     handler(id) {} // will only be called for IDs starting with `foo`
 *   }
 * }
 * ```
 */
export function prefixRegex(str: string | string[], flags?: string): RegExp {
  return new RegExp(`^${combineMultipleStrings(str)}`, flags);
}

/**
 * Constructs a RegExp that matches a value that has the specified suffix.
 *
 * This is useful for plugin hook filters.
 *
 * @param str the string to match.
 * @param flags flags for the RegExp.
 *
 * @example
 * ```ts
 * import { suffixRegex } from '@rolldown/pluginutils';
 * const plugin = {
 *   name: 'plugin',
 *   resolveId: {
 *     filter: { id: suffixRegex('.vue') },
 *     handler(id) {} // will only be called for IDs ending with `.vue`
 *   }
 * }
 * ```
 */
export function suffixRegex(str: string | string[], flags?: string): RegExp {
  return new RegExp(`${combineMultipleStrings(str)}$`, flags);
}

const escapeRegexRE = /[-/\\^$*+?.()|[\]{}]/g;
function escapeRegex(str: string): string {
  return str.replace(escapeRegexRE, '\\$&');
}
function combineMultipleStrings(
  str: string | string[],
): string {
  str = Array.isArray(str) ? str : [str];
  if (str.filter(Boolean).length === 0) {
    return '(?!)'; // matches nothing
  }
  const escapeStr = str.map(escapeRegex).join('|');
  if (escapeStr && str.length > 1) {
    return `(?:${escapeStr})`;
  }
  return escapeStr;
}

type WidenString<T> = T extends string ? string : T;

/**
 * Converts a id filter to match with an id with a query.
 *
 * @param input the id filters to convert.
 *
 * @example
 * ```ts
 * import { makeIdFiltersToMatchWithQuery } from '@rolldown/pluginutils';
 * const plugin = {
 *   name: 'plugin',
 *   transform: {
 *     filter: { id: makeIdFiltersToMatchWithQuery(['**' + '/*.js', /\.ts$/]) },
 *     // The handler will be called for IDs like:
 *     // - foo.js
 *     // - foo.js?foo
 *     // - foo.txt?foo.js
 *     // - foo.ts
 *     // - foo.ts?foo
 *     // - foo.txt?foo.ts
 *     handler(code, id) {}
 *   }
 * }
 * ```
 */
export function makeIdFiltersToMatchWithQuery<T extends string | RegExp>(
  input: T,
): WidenString<T>;
export function makeIdFiltersToMatchWithQuery<T extends string | RegExp>(
  input: readonly T[],
): WidenString<T>[];
export function makeIdFiltersToMatchWithQuery(
  input: string | RegExp | readonly (string | RegExp)[],
): string | RegExp | (string | RegExp)[];
export function makeIdFiltersToMatchWithQuery(
  input: string | RegExp | readonly (string | RegExp)[],
): string | RegExp | (string | RegExp)[] {
  if (!Array.isArray(input)) {
    return makeIdFilterToMatchWithQuery(
      // Array.isArray cannot narrow the type
      // https://github.com/microsoft/TypeScript/issues/17002
      input as Exclude<typeof input, readonly unknown[]>,
    );
  }
  return input.map((i) => makeIdFilterToMatchWithQuery(i));
}

function makeIdFilterToMatchWithQuery(
  input: string | RegExp,
): string | RegExp {
  if (typeof input === 'string') {
    return `${input}{?*,}`;
  }
  return makeRegexIdFilterToMatchWithQuery(input);
}

function makeRegexIdFilterToMatchWithQuery(input: RegExp) {
  return new RegExp(
    // replace `$` with `(?:\?.*)?$` (ignore `\$`)
    input.source.replace(/(?<!\\)\$/g, '(?:\\?.*)?$'),
    input.flags,
  );
}
