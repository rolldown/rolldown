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

type WidenString<T> = T extends string ? string : T;

/**
 * Converts a id filter to match with an id with a query.
 *
 * @param input the id filters to convert.
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
