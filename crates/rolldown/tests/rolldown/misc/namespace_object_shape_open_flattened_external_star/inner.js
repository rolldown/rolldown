// The entry chunk re-exports `node:path` directly. So the bundler does not count this `export *`
// as a source of dynamic exports. Then `inner.js` and `barrel.js` have no dynamic exports.
export * from 'node:path';
export const fromInner = 'inner';
