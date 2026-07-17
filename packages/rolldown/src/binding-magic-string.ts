import {
  BindingMagicString as NativeBindingMagicString,
  type BindingIndentOptions,
  type BindingOverwriteOptions,
  type BindingUpdateOptions,
} from './binding.cjs';

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
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeIndent = NativeBindingMagicString.prototype.indent;

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
  // Upstream accepts a legacy boolean 4th arg but spreads it away, so any boolean is
  // equivalent to the default options — normalize it so napi doesn't reject the type.
  return nativeOverwrite.call(
    this,
    start,
    end,
    content,
    typeof options === 'boolean' ? undefined : options,
  );
};

NativeBindingMagicString.prototype.update = function (
  start: any,
  end: any,
  content: any,
  options?: any,
): any {
  assertString(content, 'replacement content must be a string');
  // Upstream's legacy boolean 4th arg on update: `true` is the deprecated `storeName`
  // shorthand; `false` carries no options.
  const opts = typeof options === 'boolean' ? (options ? { storeName: true } : undefined) : options;
  return nativeUpdate.call(this, start, end, content, opts);
};

NativeBindingMagicString.prototype.indent = function (indentor?: any, options?: any): any {
  // Upstream accepts the options object as the first argument: indent({ exclude, indentStart }).
  if (indentor !== null && typeof indentor === 'object') {
    return nativeIndent.call(this, undefined, indentor);
  }
  return nativeIndent.call(this, indentor, options);
};

// Override replace/replaceAll to support RegExp patterns and function replacers.
// String patterns with string replacements delegate to the native Rust implementation.
// RegExp patterns with string replacements delegate to native replaceRegex which uses
// the regress crate for ECMAScript-compatible regex matching with capture groups.
// Function replacers run entirely on the JS side (transcribed from magic-string's
// `_replaceRegexp`/`_replaceString`/`_replaceAllString`): the match runs against
// `original` with a real JS RegExp and each changed match becomes an `overwrite()`.
// A JS callback cannot cross the FFI boundary synchronously, and this keeps regex
// semantics (lastIndex, named groups, unicode flags) exactly JavaScript's.
// eslint-disable-next-line @typescript-eslint/unbound-method -- intentionally saving refs before overriding
const nativeReplace = NativeBindingMagicString.prototype.replace;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeReplaceAll = NativeBindingMagicString.prototype.replaceAll;

type ReplacerFunction = (substring: string, ...args: any[]) => string;

// Spec `AdvanceStringIndex` (ECMA-262): the next code unit, or the next code point
// when the regexp is unicode-aware (`u`/`v`) and we're sitting on a surrogate pair.
// Used to step past a zero-width match so the exec loop below can terminate.
function advanceStringIndex(str: string, index: number, unicode: boolean): number {
  if (!unicode || index + 1 >= str.length) return index + 1;
  const first = str.charCodeAt(index);
  if (first < 0xd800 || first > 0xdbff) return index + 1;
  const second = str.charCodeAt(index + 1);
  if (second < 0xdc00 || second > 0xdfff) return index + 1;
  return index + 2;
}

interface Edit {
  start: number;
  end: number;
  value: string;
}

// A UTF-16 index sits *inside* a surrogate pair when the unit before it is a high
// surrogate and the unit at it is a low surrogate. Overwriting across such a boundary
// would split an indivisible UTF-8 character, which our native store cannot represent.
function isSurrogatePairInterior(str: string, index: number): boolean {
  if (index <= 0 || index >= str.length) return false;
  const prev = str.charCodeAt(index - 1);
  const cur = str.charCodeAt(index);
  return prev >= 0xd800 && prev <= 0xdbff && cur >= 0xdc00 && cur <= 0xdfff;
}

// Upstream edits a UTF-16 rope one code unit at a time, so a non-Unicode regexp like
// `/./g` can replace each half of a surrogate pair independently. Our UTF-8 store cannot
// address a lone surrogate, but when *both* halves are replaced the two adjacent edits
// coalesce into one overwrite of the whole character (`🤷` -> `"x" + "x"` = `"xx"`),
// reproducing upstream without ever splitting the string. When only one half is replaced
// the result would contain a lone surrogate, which is unrepresentable — so we throw.
// `edits` must be in ascending, non-overlapping order (as regex matches always are).
function coalesceSurrogateEdits(original: string, edits: Edit[]): Edit[] {
  const merged: Edit[] = [];
  for (let i = 0; i < edits.length; ) {
    let { start, end, value } = edits[i];
    i++;
    if (isSurrogatePairInterior(original, start)) {
      // The high half preceding `start` is untouched, so overwriting from here would
      // strand it as a lone surrogate.
      throw new Error(
        `Cannot replace a range that splits a surrogate pair at UTF-16 index ${start}; ` +
          'use a Unicode-aware RegExp (u or v flag)',
      );
    }
    while (isSurrogatePairInterior(original, end)) {
      // `end` bisects a pair: the low half is only representable if the adjacent edit
      // consumes it too. Merge that edit in; otherwise the low half is stranded.
      const next = edits[i];
      if (!next || next.start !== end) {
        throw new Error(
          `Cannot replace a range that splits a surrogate pair at UTF-16 index ${end}; ` +
            'use a Unicode-aware RegExp (u or v flag)',
        );
      }
      value += next.value;
      end = next.end;
      i++;
    }
    merged.push({ start, end, value });
  }
  return merged;
}

// The callback sees the same arguments as `String.prototype.replace`:
// (match, p1, ..., pn, offset, string, groups). Like upstream, `groups` is always
// passed (undefined when the pattern has no named groups), and an overwrite is only
// issued when the replacement differs from the matched text, so a no-op replacer
// leaves `hasChanged()` false. Matches are collected before any callback/overwrite runs,
// so `coalesceSurrogateEdits` can see the full set (needed to merge surrogate-pair halves).
function replaceRegexWithFunction(
  s: any,
  searchValue: RegExp,
  replacement: ReplacerFunction,
): void {
  const original: string = s.original;
  const matches: RegExpMatchArray[] = [];
  if (searchValue.global) {
    // Upstream exec-loops from the regex's current lastIndex (it does not pre-reset
    // the way `String.prototype.replace` does) and leaves it at 0 on exhaustion.
    const fullUnicode = searchValue.unicode || searchValue.unicodeSets;
    let match: RegExpExecArray | null;
    while ((match = searchValue.exec(original))) {
      matches.push(match);
      // A zero-width match (e.g. `/(?=a)/g`, `/^/gm`) leaves lastIndex unchanged, so
      // the next exec() would re-match the same position forever. Upstream omits this
      // guard and hangs; advance lastIndex ourselves per the spec instead.
      if (match[0].length === 0) {
        searchValue.lastIndex = advanceStringIndex(original, searchValue.lastIndex, fullUnicode);
      }
    }
  } else {
    const match = original.match(searchValue);
    if (match) {
      matches.push(match);
    }
  }
  const edits: Edit[] = [];
  for (const match of matches) {
    if (match.index == null) continue;
    const value = replacement(
      ...(match as [string, ...string[]]),
      match.index,
      original,
      match.groups,
    );
    if (value !== match[0]) {
      edits.push({ start: match.index, end: match.index + match[0].length, value });
    }
  }
  for (const { start, end, value } of coalesceSurrogateEdits(original, edits)) {
    s.overwrite(start, end, value);
  }
}

NativeBindingMagicString.prototype.replace = function (
  searchValue: string | RegExp,
  replacement: string | ReplacerFunction,
): any {
  if (typeof searchValue === 'string') {
    if (typeof replacement === 'function') {
      // Upstream `_replaceString`: first occurrence only.
      const original: string = (this as any).original;
      const index = original.indexOf(searchValue);
      if (index !== -1) {
        const value = replacement(searchValue, index, original);
        if (searchValue !== value) {
          this.overwrite(index, index + searchValue.length, value);
        }
      }
      return this;
    }
    return nativeReplace.call(this, searchValue, replacement);
  }
  if (typeof replacement === 'function') {
    replaceRegexWithFunction(this, searchValue, replacement);
    return this;
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
  replacement: string | ReplacerFunction,
): any {
  if (typeof searchValue === 'string') {
    if (typeof replacement === 'function') {
      // Upstream `_replaceAllString`: every occurrence, non-overlapping.
      const original: string = (this as any).original;
      const stringLength = searchValue.length;
      if (stringLength === 0) {
        // An empty search matches at every index without advancing, so upstream's
        // indexOf loop spins forever. A zero-length overwrite is unsupported anyway,
        // so reject it exactly like the native path rather than hanging.
        throw new Error(
          'Cannot overwrite a zero-length range – use appendLeft or prependRight instead',
        );
      }
      for (
        let index = original.indexOf(searchValue);
        index !== -1;
        index = original.indexOf(searchValue, index + stringLength)
      ) {
        const previous = original.slice(index, index + stringLength);
        const value = replacement(previous, index, original);
        if (previous !== value) {
          this.overwrite(index, index + stringLength, value);
        }
      }
      return this;
    }
    return nativeReplaceAll.call(this, searchValue, replacement);
  }
  if (!searchValue.global) {
    throw new TypeError(
      'MagicString.prototype.replaceAll called with a non-global RegExp argument',
    );
  }
  if (typeof replacement === 'function') {
    replaceRegexWithFunction(this, searchValue, replacement);
    return this;
  }
  searchValue.lastIndex = 0;
  (this as any).replaceRegex(searchValue, replacement);
  searchValue.lastIndex = 0;
  return this;
};

export interface RolldownMagicString extends NativeBindingMagicString {
  readonly isRolldownMagicString: true;
  /**
   * Accepts a string or RegExp pattern. String replacements support `$&`, `$$`, and `$N`
   * substitutions; a function replacer is called like `String.prototype.replace`'s.
   */
  replace(from: string | RegExp, to: string | ReplacerFunction): this;
  /**
   * Accepts a string or RegExp pattern. RegExp must have the global (`g`) flag.
   * A function replacer is called like `String.prototype.replace`'s, once per match.
   */
  replaceAll(from: string | RegExp, to: string | ReplacerFunction): this;
  /**
   * The 4th argument also accepts the deprecated boolean form. `overwrite(s, e, c, true)`
   * spreads to the default options (upstream ignores it); `update(s, e, c, true)` is
   * equivalent to `{ storeName: true }`.
   */
  overwrite(
    start: number,
    end: number,
    content: string,
    options?: boolean | BindingOverwriteOptions | null,
  ): this;
  update(
    start: number,
    end: number,
    content: string,
    options?: boolean | BindingUpdateOptions | null,
  ): this;
  /**
   * The options object may also be passed as the first argument: `indent({ exclude, indentStart })`.
   * `indentStart: false` leaves the first line un-indented.
   */
  indent(
    indentor?: string | BindingIndentOptions | null,
    options?: BindingIndentOptions | null,
  ): this;
  /**
   * Rolldown-only; not part of the magic-string API. `relocate` is the native name behind
   * the standard {@link RolldownMagicString.move} alias — prefer `move`.
   *
   * @internal
   * @deprecated Use `move` instead.
   */
  relocate(start: number, end: number, to: number): this;
  /**
   * Rolldown-only native regex primitive that `replace`/`replaceAll` call internally.
   * Not part of the magic-string API — prefer `replace(regexp, …)` / `replaceAll(regexp, …)`.
   *
   * @internal
   * @deprecated Use `replace`/`replaceAll` with a RegExp instead.
   */
  replaceRegex(from: RegExp, to: string): number;
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
