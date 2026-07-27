// Cross-chunk dynamic importer: loading `b` must initialize `target` without running
// entry `a`'s side effects.
(globalThis.log ??= []).push('b');
export const bTargetPromise = import('./target.js');
