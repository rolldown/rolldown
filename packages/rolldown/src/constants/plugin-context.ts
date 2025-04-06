/**
 * If Composed plugins call `this.resolve` with `skipSelf: true`, the composed plugins will be skipped as a whole.
 * To prevent that, we use this symbol to store the actual caller of `this.resolve` with `skipSelf: true`. And we
 * will modify the skipSelf option to `false` and use this symbol to skip the caller itself in the composed plugins
 * internally.
 */
export const SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF: unique symbol = Symbol(
  'plugin-context-resolve-caller',
);
