import { BindingMagicString as NativeBindingMagicString } from './binding.cjs';

// Set `isRolldownMagicString` so external packages (e.g. rolldown-string) can
// detect native BindingMagicString instances without importing rolldown:
//   obj.isRolldownMagicString === true
// This replaces the fragile `obj.constructor.name` check which breaks with
// minification or bundling.
Object.defineProperty(NativeBindingMagicString.prototype, 'isRolldownMagicString', {
  value: true,
  writable: false,
  configurable: false,
});

// Validate content type to match JS magic-string behavior.
// napi-rs throws a generic Error on type mismatch, but JS magic-string throws TypeError.
function assertString(content: unknown, msg: string): asserts content is string {
  if (typeof content !== 'string') throw new TypeError(msg);
}

// Save native method refs before overriding.
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeAppend = NativeBindingMagicString.prototype.append;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativePrepend = NativeBindingMagicString.prototype.prepend;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeAppendLeft = NativeBindingMagicString.prototype.appendLeft;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeAppendRight = NativeBindingMagicString.prototype.appendRight;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativePrependLeft = NativeBindingMagicString.prototype.prependLeft;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativePrependRight = NativeBindingMagicString.prototype.prependRight;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeOverwrite = NativeBindingMagicString.prototype.overwrite;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeUpdate = NativeBindingMagicString.prototype.update;

NativeBindingMagicString.prototype.append = function (content: any): any {
  assertString(content, 'outro content must be a string');
  return nativeAppend.call(this, content);
};

NativeBindingMagicString.prototype.prepend = function (content: any): any {
  assertString(content, 'outro content must be a string');
  return nativePrepend.call(this, content);
};

NativeBindingMagicString.prototype.appendLeft = function (index: any, content: any): any {
  assertString(content, 'inserted content must be a string');
  return nativeAppendLeft.call(this, index, content);
};

NativeBindingMagicString.prototype.appendRight = function (index: any, content: any): any {
  assertString(content, 'inserted content must be a string');
  return nativeAppendRight.call(this, index, content);
};

NativeBindingMagicString.prototype.prependLeft = function (index: any, content: any): any {
  assertString(content, 'inserted content must be a string');
  return nativePrependLeft.call(this, index, content);
};

NativeBindingMagicString.prototype.prependRight = function (index: any, content: any): any {
  assertString(content, 'inserted content must be a string');
  return nativePrependRight.call(this, index, content);
};

NativeBindingMagicString.prototype.overwrite = function (
  start: any,
  end: any,
  content: any,
  options?: any,
): any {
  assertString(content, 'replacement content must be a string');
  return nativeOverwrite.call(this, start, end, content, options);
};

NativeBindingMagicString.prototype.update = function (
  start: any,
  end: any,
  content: any,
  options?: any,
): any {
  assertString(content, 'replacement content must be a string');
  return nativeUpdate.call(this, start, end, content, options);
};

// Override replace/replaceAll to support RegExp patterns.
// String patterns delegate to the native Rust implementation.
// RegExp patterns delegate to native replaceRegex which uses the regress crate
// for ECMAScript-compatible regex matching with capture groups.
// eslint-disable-next-line @typescript-eslint/unbound-method -- intentionally saving refs before overriding
const nativeReplace = NativeBindingMagicString.prototype.replace;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeReplaceAll = NativeBindingMagicString.prototype.replaceAll;

// Function replacements cannot be handled by the native binding, which only
// accepts string replacements. Instead, the match runs in JS — with `String`
// semantics for flags, capture groups and `lastIndex` — and each result is
// applied through the positional native APIs. This mirrors magic-string's
// `_replaceRegexp` / `_replaceString` / `_replaceAllString`, minus the
// removed-content checks that need chunk internals the binding does not expose.
export type ReplacementFunction = (substring: string, ...args: any[]) => string;

// Native positional APIs take u32 indices and apply `offset`, so a match
// before the offset cannot be expressed — write original-string indices with
// `offset` temporarily zeroed.
function writeWithZeroOffset(s: NativeBindingMagicString, write: () => void): void {
  const offset = s.offset;
  s.offset = 0;
  try {
    write();
  } finally {
    s.offset = offset;
  }
}

function getRegexReplacement(
  match: RegExpMatchArray,
  original: string,
  replacement: ReplacementFunction,
): string {
  return match.groups === undefined
    ? replacement(match[0], ...match.slice(1), match.index ?? 0, original)
    : replacement(match[0], ...match.slice(1), match.index ?? 0, original, match.groups);
}

function replaceRegexWithFunction(
  s: NativeBindingMagicString,
  searchValue: RegExp,
  replacement: ReplacementFunction,
): void {
  const original = s.original;
  const replaceMatch = (match: RegExpMatchArray) => {
    // `match.index` is always defined for results of `matchAll` / `match`.
    const index = match.index;
    if (index === undefined) return;
    const content = getRegexReplacement(match, original, replacement);
    if (content === match[0]) return; // ignore non-changed replacements
    if (match[0].length === 0) {
      writeWithZeroOffset(s, () => nativeAppendRight.call(s, index, content));
    } else {
      writeWithZeroOffset(s, () =>
        nativeOverwrite.call(s, index, index + match[0].length, content),
      );
    }
  };
  if (searchValue.global) {
    searchValue.lastIndex = 0;
    for (const match of original.matchAll(searchValue)) replaceMatch(match);
  } else {
    // `String.prototype.match` keeps JS semantics for sticky regexes.
    const match = original.match(searchValue);
    if (match) replaceMatch(match);
  }
}

function replaceStringWithFunction(
  s: NativeBindingMagicString,
  searchValue: string,
  replacement: ReplacementFunction,
  replaceAll: boolean,
): void {
  const original = s.original;
  if (searchValue === '') {
    // An empty pattern matches at every index; magic-string inserts at each.
    const last = replaceAll ? original.length : 0;
    for (let index = 0; index <= last; index++) {
      const content = replacement('', index, original);
      if (content !== '') writeWithZeroOffset(s, () => nativeAppendRight.call(s, index, content));
    }
    return;
  }
  for (
    let index = original.indexOf(searchValue);
    index !== -1;
    index = original.indexOf(searchValue, index + searchValue.length)
  ) {
    const content = replacement(searchValue, index, original);
    if (content !== searchValue) {
      writeWithZeroOffset(s, () =>
        nativeOverwrite.call(s, index, index + searchValue.length, content),
      );
    }
    if (!replaceAll) return;
  }
}

NativeBindingMagicString.prototype.replace = function (
  searchValue: string | RegExp,
  replacement: string | ReplacementFunction,
): any {
  if (typeof replacement === 'function') {
    if (typeof searchValue === 'string') {
      replaceStringWithFunction(this, searchValue, replacement, false);
    } else {
      replaceRegexWithFunction(this, searchValue, replacement);
    }
    return this;
  }
  if (typeof searchValue === 'string') {
    return nativeReplace.call(this, searchValue, replacement);
  }
  // For global regexes, JS resets lastIndex to 0 before matching.
  if (searchValue.global) {
    searchValue.lastIndex = 0;
  }
  // replaceRegex returns the UTF-16 offset past the last match, or -1 if no match.
  const lastMatchEnd: number = (this as any).replaceRegex(searchValue, replacement);
  // Update lastIndex to match JS semantics:
  // - Global: reset to 0 (exec loop exhaustion)
  // - Non-global sticky: advance to match end, or reset to 0 on miss
  // - Non-global non-sticky: lastIndex is not modified by .replace()
  if (searchValue.global) {
    searchValue.lastIndex = 0;
  } else if (searchValue.sticky) {
    searchValue.lastIndex = lastMatchEnd === -1 ? 0 : lastMatchEnd;
  }
  return this;
};

NativeBindingMagicString.prototype.replaceAll = function (
  searchValue: string | RegExp,
  replacement: string | ReplacementFunction,
): any {
  if (typeof replacement === 'function') {
    if (typeof searchValue === 'string') {
      replaceStringWithFunction(this, searchValue, replacement, true);
      return this;
    }
    if (!searchValue.global) {
      throw new TypeError(
        'MagicString.prototype.replaceAll called with a non-global RegExp argument',
      );
    }
    replaceRegexWithFunction(this, searchValue, replacement);
    return this;
  }
  if (typeof searchValue === 'string') {
    return nativeReplaceAll.call(this, searchValue, replacement);
  }
  if (!searchValue.global) {
    throw new TypeError(
      'MagicString.prototype.replaceAll called with a non-global RegExp argument',
    );
  }
  searchValue.lastIndex = 0;
  (this as any).replaceRegex(searchValue, replacement);
  searchValue.lastIndex = 0;
  return this;
};

export interface RolldownMagicString extends NativeBindingMagicString {
  readonly isRolldownMagicString: true;
  /**
   * Accepts a string or RegExp pattern with a string replacement (`$&`, `$$`,
   * and `$N` substitutions are supported), or a replacer function invoked like
   * `String.prototype.replace`.
   */
  replace(from: string | RegExp, to: string | ReplacementFunction): this;
  /**
   * Accepts a string or RegExp pattern (RegExp must have the global (`g`)
   * flag) with a string replacement, or a replacer function.
   */
  replaceAll(from: string | RegExp, to: string | ReplacementFunction): this;
}

type RolldownMagicStringConstructor = Omit<typeof NativeBindingMagicString, 'prototype'> & {
  new (...args: ConstructorParameters<typeof NativeBindingMagicString>): RolldownMagicString;
  prototype: RolldownMagicString;
};

/**
 * A native MagicString implementation powered by Rust.
 *
 * @experimental
 */
export const RolldownMagicString = NativeBindingMagicString as RolldownMagicStringConstructor;
