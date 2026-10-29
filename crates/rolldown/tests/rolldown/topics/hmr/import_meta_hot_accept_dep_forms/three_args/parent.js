import { count } from './child.js';

globalThis.three_argsParentRuns = (globalThis.three_argsParentRuns ?? 0) + 1;
globalThis.three_argsSaw = count;

import.meta.hot.accept(
  './child.js',
  (mod) => {
    globalThis.three_argsSaw = mod.count;
  },
  'extra',
);
