// Same-chunk dynamic importer: the group places `target.js` next to this entry's
// implementation, so this `import()` could collapse — but b.js's cross-chunk import
// must keep the facade for everyone.
import { value } from './target.js';
(globalThis.log ??= []).push('a:' + value);
export const aTargetPromise = import('./target.js');
