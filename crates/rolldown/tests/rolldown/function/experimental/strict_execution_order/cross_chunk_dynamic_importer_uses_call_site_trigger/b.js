import { setValue, unrelated, valueAlias } from './bridge.js';

// Cross-chunk dynamic importer: loading `b` must initialize `target` without running
// entry `a`'s side effects. These static reads retain exports that the dynamic import
// does not use, so they must not widen `target`'s simulated facade namespace.
(globalThis.log ??= []).push('b');
setValue(1);
if (valueAlias !== 1 || unrelated() !== 'unrelated') {
  throw new Error('retained barrel exports have the wrong value');
}
export const bTargetPromise = import('./target.js').then(({ value }) => ({ value }));
