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

NativeBindingMagicString.prototype.replace = function (
  searchValue: string | RegExp,
  replacement: string,
): any {
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
  replacement: string,
): any {
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
  /** Accepts a string or RegExp pattern. RegExp supports `$&`, `$$`, and `$N` substitutions. */
  replace(from: string | RegExp, to: string): this;
  /** Accepts a string or RegExp pattern. RegExp must have the global (`g`) flag. */
  replaceAll(from: string | RegExp, to: string): this;
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
