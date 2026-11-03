export function x() {
  return 1;
}

// Neither star contributes to this module's namespace: the first `x` is shadowed by the local
// declaration above, while `export *` never forwards the second module's default export.
export * from './shadowed.js';
export * from './default-only.js';
