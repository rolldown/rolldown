// `export { barrel }` reads the whole namespace object of `barrel.js`, so that object stays in the
// output. `export * from './barrel.js'` puts `barrel.js` and `inner.js` in the `export *` chain of
// the entry. So the entry chunk re-exports `node:path` directly: their `export * from 'node:path'`
// is an entry-level external `export *`.
import * as barrel from './barrel.js';

export * from './barrel.js';
export { barrel };
