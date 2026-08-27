import { BindingMagicString as NativeBindingMagicString } from './binding.cjs';

// Set `isRolldownMagicString` so external packages (e.g. rolldown-string) can
// detect native BindingMagicString instances without importing rolldown:
//   obj.isRolldownMagicString === true
// This replaces the fragile `obj.constructor.name` check which breaks with
// minification or bundling. It must be a *data property on the prototype* (readable via
// `Prototype.isRolldownMagicString` too, without a live instance), which a napi getter
// cannot express — reading a native accessor through the bare prototype throws.
Object.defineProperty(NativeBindingMagicString.prototype, 'isRolldownMagicString', {
  value: true,
  writable: false,
  configurable: false,
});

// This wrapper only overrides `replace`/`replaceAll` — the parts that inherently need
// JavaScript: matching with a real JS RegExp (V8 `exec`/`lastIndex`/Unicode semantics that
// the native regex engines cannot replicate exactly) and invoking user callbacks. All
// validation (TypeError on non-string content), the `isRolldownMagicString` brand, offset
// application, and chunk-state queries live on the native side.
//
// String patterns with string replacements delegate to the native Rust implementation.
// RegExp patterns with string replacements delegate to native `replaceRegex`, which uses
// ECMAScript-compatible native regex matching and updates the caller's `lastIndex` itself.
// Function replacers run their match loop here (transcribed from magic-string's
// `_replaceRegexp`/`_replaceString`/`_replaceAllString`), and each changed match becomes a
// native `overwrite()`.
// eslint-disable-next-line @typescript-eslint/unbound-method -- intentionally saving refs before overriding
const nativeReplace = NativeBindingMagicString.prototype.replace;
// eslint-disable-next-line @typescript-eslint/unbound-method
const nativeReplaceAll = NativeBindingMagicString.prototype.replaceAll;

type ReplacerFunction = (substring: string, ...args: any[]) => string;

// Upstream's exec loop writes `lastIndex` while matching — before any overwrite — so a
// global or sticky regex whose `lastIndex` is read-only throws V8's TypeError with the
// source untouched. Native matching bypasses the JS property (and napi's property set is
// silent on read-only properties), so replicate that first write here before delegating:
// assigning the current value back is a no-op when writable and throws exactly V8's
// "Cannot assign to read only property 'lastIndex'" when not. Non-global non-sticky
// regexes never have `lastIndex` written, matching `RegExpBuiltinExec`.
function assertLastIndexWritable(searchValue: RegExp): void {
  if (searchValue.global || searchValue.sticky) {
    const lastIndex = searchValue.lastIndex;
    searchValue.lastIndex = lastIndex;
  }
}

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

interface RawMatch {
  // The UTF-16 start index, the matched text, and the full argument list the replacer
  // callback receives (regex: match, ...captures, index, string, groups; string search:
  // matched, index, string). Matches are collected first, but the callback runs later, at
  // apply time, so a stateful replacer observes edits from earlier matches (as upstream does).
  index: number;
  matched: string;
  args: any[];
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

// A well-formed UTF-16 string has every high surrogate immediately followed by a low
// surrogate and contains no lone low surrogates. Our UTF-8 store can only hold well-formed
// text, so a coalesced overwrite whose value would strand a lone surrogate must be rejected.
function isWellFormedUtf16(str: string): boolean {
  for (let i = 0; i < str.length; i++) {
    const c = str.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      // High surrogate: must be immediately followed by a low surrogate.
      if (i + 1 >= str.length) return false;
      const next = str.charCodeAt(i + 1);
      if (next < 0xdc00 || next > 0xdfff) return false;
      i++;
    } else if (c >= 0xdc00 && c <= 0xdfff) {
      // Lone low surrogate.
      return false;
    }
  }
  return true;
}

// Applies a function replacer's matches to `s`. Upstream magic-string interleaves
// callback -> overwrite per match: a stateful replacer (`() => s.toString()`) observes
// earlier edits, and a failing overwrite throws after that match's callback with every
// earlier edit still applied. We reproduce that ordering — including the failure
// semantics: an error mid-run (splitting a previously edited chunk, a zero-length
// overwrite, an unrepresentable lone surrogate) propagates with the earlier matches'
// edits in place. Deliberately no rollback: un-overwriting cannot restore chunk state, so
// a "restored" instance would lie about its history while looking untouched.
//
// Surrogate pairs are where we diverge mechanically. A non-Unicode regexp like `/./g`
// matches each UTF-16 half of an astral character separately; upstream's UTF-16 rope
// edits the halves independently, but our UTF-8 store cannot address a lone surrogate, so
// a match boundary inside a pair grows to the enclosing character: the untouched high
// half is kept in front of an interior start, and an interior end folds in either the
// untouched low half or the adjacent match starting there — consuming further matches
// (and running their callbacks, in order) while the combined edit still ends inside a
// pair. Whenever the edit-so-far plus the untouched low half forms valid UTF-16 it is
// applied *before* the adjacent match's callback runs, so stateful replacers observe the
// same intermediate state as upstream; when both halves are being rewritten there is no
// representable intermediate, and the adjacent callback sees the pre-edit state instead.
// A combined edit that still strands a lone surrogate once complete is unrepresentable in
// UTF-8 and throws. Because upstream *splits* its rope at every overwrite boundary —
// throwing "Cannot split a chunk that has already been edited" on previously edited
// content — each pair-interior boundary the widening smooths over is first checked via
// the native `assertCanSplitAt`, which surfaces that error exactly where upstream raises
// it. `matches` must be ascending and non-overlapping (as regex and string-search matches
// always are).
function applyFunctionReplacements(
  s: any,
  original: string,
  matches: RawMatch[],
  replacement: ReplacerFunction,
): void {
  const results: string[] = [];
  let nextToRun = 0;
  // Run callbacks lazily (the folding below consumes adjacent matches) but always in
  // ascending order and at most once each, preserving upstream's call ordering.
  const valueAt = (i: number): string => {
    while (nextToRun <= i) {
      const m = matches[nextToRun];
      const result = replacement(...(m.args as [string, ...any[]]));
      // Upstream validates the result inside overwrite()/update(); the folding below
      // concatenates first, which would coerce a non-string (e.g. a String object) before
      // overwrite could reject it. Validate when the result is produced — the same moment
      // upstream consumes it, with the same error.
      if (typeof result !== 'string') {
        throw new TypeError('replacement content must be a string');
      }
      results[nextToRun] = result;
      nextToRun++;
    }
    return results[i];
  };
  const splitError = (index: number): Error =>
    new Error(
      `Cannot replace a range that splits a surrogate pair at UTF-16 index ${index}; ` +
        'replace the whole character or use a Unicode-aware RegExp (u or v flag)',
    );
  // Runs a native positional call with `s.offset` forced to `pinned` for its duration.
  // The folding below decides boundaries at the shifted positions and must apply its edits
  // at those same positions — but a callback peeked at mid-fold may reassign the public
  // `offset` between the decision and the call. Nothing observes `s.offset` inside `fn`,
  // so the swap is invisible to user code.
  const pinOffset = <T>(pinned: number, fn: () => T): T => {
    const live: number = s.offset;
    if (live === pinned) return fn();
    s.offset = pinned;
    try {
      return fn();
    } finally {
      s.offset = live;
    }
  };

  for (let i = 0; i < matches.length; i++) {
    const m = matches[i];
    const value = valueAt(i);
    if (value === m.matched) continue; // a no-op replacer issues no overwrite

    // Match indices live in the unshifted coordinate space of `original` (upstream runs
    // indexOf/exec the same way), but every edit method shifts its indices by `offset`
    // before touching the store. Surrogate-boundary decisions concern the characters the
    // edit actually lands on, so they are made at the *shifted* positions. Upstream reads
    // the live `offset` inside each edit call — i.e. after that match's callback, which
    // may have reassigned it — so capture it here, per match, and pin this match's native
    // calls to it. A cluster only merges adjacent matches while the offset is unchanged;
    // when a peeked callback moves it, the cluster closes and the adjacent match is
    // applied independently at its own captured offset (see below).
    const offset: number = s.offset;

    let start = m.index;
    let end = m.index + m.matched.length;

    // A zero-width match is an insertion point; upstream issues overwrite(index, index,
    // value), which rejects the zero-length range before it splits anything.
    if (start === end) {
      s.overwrite(start, end, value); // throws "Cannot overwrite a zero-length range"
      continue;
    }

    // Surface the split errors upstream would raise at pair-interior boundaries (start
    // first, as upstream splits it first) before widening hides them.
    const startInterior = isSurrogatePairInterior(original, start + offset);
    if (startInterior) pinOffset(offset, () => s.assertCanSplitAt(start));
    if (isSurrogatePairInterior(original, end + offset)) {
      pinOffset(offset, () => s.assertCanSplitAt(end));
    }

    let text = value;
    if (startInterior) {
      // Keep the untouched high half in front (a changed high half would have consumed
      // this match while resolving its own low half) and align to the character start.
      text = original[start + offset - 1] + text;
      start -= 1;
    }

    // Fold pair interiors at `end` until the combined edit closes on a character
    // boundary. `appliedEnd`/`appliedText` track a tentatively applied representable
    // prefix, so the final overwrite is skipped when it is already in place.
    let appliedEnd = -1;
    let appliedText = '';
    while (isSurrogatePairInterior(original, end + offset)) {
      const settled = text + original[end + offset]; // close with the untouched low half
      const canSettle = isWellFormedUtf16(settled);
      // Only an adjacent *non-empty* match continues the combined edit. A zero-width match
      // is an insertion point whose overwrite upstream rejects as zero-length; folding its
      // replacement into the neighbouring character edit would bypass that rejection, so it
      // is left for the outer loop's zero-width guard to process independently.
      const next =
        i + 1 < matches.length && matches[i + 1].index === end && matches[i + 1].matched.length > 0
          ? matches[i + 1]
          : undefined;

      if (next === undefined) {
        if (!canSettle) throw splitError(m.index);
        text = settled;
        end += 1; // the unit after a low surrogate is never another pair interior
        break;
      }

      // An adjacent match continues the combined edit. Upstream applies the current edit
      // before running that match's callback, so when the settled projection is
      // representable, apply it now — the callback then observes upstream's exact
      // intermediate state (replaced halves in place, low half still untouched).
      if (canSettle) {
        pinOffset(offset, () => s.overwrite(start, end + 1, settled));
        appliedEnd = end + 1;
        appliedText = settled;
      }
      const nextValue = valueAt(i + 1);
      if (nextValue === next.matched || s.offset !== offset) {
        // The adjacent match is a no-op (upstream leaves its range untouched), or its
        // callback reassigned `offset` — upstream then applies that match at the *new*
        // offset, i.e. at a different location that no longer continues this character.
        // Either way the combined edit closes with the settled text, and the outer loop
        // revisits the adjacent match: a no-op is skipped, an offset-moved match is
        // applied independently at its own captured offset.
        if (!canSettle) throw splitError(m.index);
        text = settled;
        end += 1;
        break;
      }
      // Merge the adjacent changed match and keep folding; its own end may sit inside a
      // further pair, whose split gets checked just like the original boundaries.
      i++;
      text += nextValue;
      end = matches[i].index + matches[i].matched.length;
      if (isSurrogatePairInterior(original, end + offset)) {
        pinOffset(offset, () => s.assertCanSplitAt(end));
      }
    }

    if (!isWellFormedUtf16(text)) throw splitError(m.index);
    if (end !== appliedEnd || text !== appliedText) {
      pinOffset(offset, () => s.overwrite(start, end, text));
    }
  }
}

// The callback sees the same arguments as `String.prototype.replace`:
// (match, p1, ..., pn, offset, string, groups). Like upstream, `groups` is always passed
// (undefined when the pattern has no named groups), and an overwrite is only issued when the
// replacement differs from the matched text, so a no-op replacer leaves `hasChanged()`
// false. Matches are collected up front (against `original`, which never changes), but the
// callbacks run later inside `applyFunctionReplacements`, in order, each immediately before
// its overwrite — so a stateful replacer sees earlier edits exactly as upstream does.
function replaceRegexWithFunction(
  s: any,
  searchValue: RegExp,
  replacement: ReplacerFunction,
): void {
  const original: string = s.original;
  const matches: RawMatch[] = [];
  const collect = (match: RegExpMatchArray): void => {
    if (match.index == null) return;
    matches.push({
      index: match.index,
      matched: match[0],
      args: [...match, match.index, original, match.groups],
    });
  };
  if (searchValue.global) {
    // Upstream exec-loops from the regex's current lastIndex (it does not pre-reset
    // the way `String.prototype.replace` does) and leaves it at 0 on exhaustion.
    const fullUnicode = searchValue.unicode || searchValue.unicodeSets;
    let match: RegExpExecArray | null;
    while ((match = searchValue.exec(original))) {
      collect(match);
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
      collect(match);
    }
  }
  applyFunctionReplacements(s, original, matches, replacement);
}

NativeBindingMagicString.prototype.replace = function (
  searchValue: string | RegExp,
  replacement: string | ReplacerFunction,
): any {
  if (typeof searchValue === 'string') {
    if (typeof replacement === 'function') {
      // Upstream `_replaceString`: first occurrence only. Route through the shared applier so
      // a match that bisects a surrogate pair is coalesced instead of hitting overwrite()
      // with a half-character range.
      const original: string = (this as any).original;
      const index = original.indexOf(searchValue);
      if (index !== -1) {
        applyFunctionReplacements(
          this,
          original,
          [{ index, matched: searchValue, args: [searchValue, index, original] }],
          replacement,
        );
      }
      return this;
    }
    return nativeReplace.call(this, searchValue, replacement);
  }
  if (typeof replacement === 'function') {
    replaceRegexWithFunction(this, searchValue, replacement);
    return this;
  }
  // Native replaceRegex updates the caller's `lastIndex` itself, with
  // `String.prototype.replace` semantics — but a read-only `lastIndex` must throw before
  // anything is edited, as upstream's exec loop would.
  assertLastIndexWritable(searchValue);
  (this as any).replaceRegex(searchValue, replacement);
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
      // Collect the (non-overlapping) occurrences against `original`, then apply through the
      // shared applier so surrogate-splitting matches coalesce and stateful callbacks still
      // observe earlier edits (each callback runs just before its overwrite).
      const matches: RawMatch[] = [];
      for (
        let index = original.indexOf(searchValue);
        index !== -1;
        index = original.indexOf(searchValue, index + stringLength)
      ) {
        matches.push({ index, matched: searchValue, args: [searchValue, index, original] });
      }
      applyFunctionReplacements(this, original, matches, replacement);
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
  assertLastIndexWritable(searchValue);
  (this as any).replaceRegex(searchValue, replacement);
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
