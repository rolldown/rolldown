// Same-chunk dynamic importer: the group places `target.js` in this entry's chunk, so
// this `import()` alone could collapse — but b.js's cross-chunk import must keep the
// facade for everyone.
(globalThis.log ??= []).push('a');
export const aTargetPromise = import('./target.js');
