// Workerd bundles alias `src/binding-magic-string.ts` to this stub. The real
// module mutates `BindingMagicString.prototype` at module evaluation, which
// requires an ambient binding instance that workerd bundles only have while a
// managed build is active.
//
// The stub keeps the two behaviors the reused pipeline depends on:
// - `value instanceof RolldownMagicString` must be safe and false, so the
//   bindingify hook-result paths take the plain `{ code }` object route.
// - Constructing one reports a clear unsupported-feature error.
import type { RolldownMagicString as RealRolldownMagicString } from '../binding-magic-string';

export type { RolldownMagicString as RolldownMagicStringType } from '../binding-magic-string';

class UnsupportedWorkerdMagicString {
  constructor() {
    throw new Error(
      'MagicString is not supported in the workerd build yet; return `{ code }` from hooks instead',
    );
  }

  static [Symbol.hasInstance](): boolean {
    return false;
  }
}

export const RolldownMagicString: typeof RealRolldownMagicString =
  UnsupportedWorkerdMagicString as unknown as typeof RealRolldownMagicString;
