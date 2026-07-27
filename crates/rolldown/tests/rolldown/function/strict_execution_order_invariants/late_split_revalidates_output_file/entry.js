import { a } from './lib.js';

(globalThis.__events ??= []).push('entry ' + a);

// Two-argument form on purpose: an options import is never rewritten, so the call site cannot
// carry the trigger and lowering has to revive `lib.js`'s facade.
import('./lib.js', {}).then((mod) => {
  (globalThis.__events ??= []).push('lazy ' + mod.a);
});
