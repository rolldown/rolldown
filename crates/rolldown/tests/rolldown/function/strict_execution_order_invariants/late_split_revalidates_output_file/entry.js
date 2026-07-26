import { a } from './lib.js';

(globalThis.__events ??= []).push('entry ' + a);

import('./lib.js').then((mod) => {
  (globalThis.__events ??= []).push('lazy ' + mod.a);
});
