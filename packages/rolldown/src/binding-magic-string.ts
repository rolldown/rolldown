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

export interface BindingMagicString extends NativeBindingMagicString {
  readonly isRolldownMagicString: true;
}

type BindingMagicStringConstructor = Omit<typeof NativeBindingMagicString, 'prototype'> & {
  new (...args: ConstructorParameters<typeof NativeBindingMagicString>): BindingMagicString;
  prototype: BindingMagicString;
};

/**
 * A native MagicString implementation powered by Rust.
 *
 * Publicly exported as {@linkcode RolldownMagicString}.
 *
 * @experimental
 */
export const BindingMagicString = NativeBindingMagicString as BindingMagicStringConstructor;
