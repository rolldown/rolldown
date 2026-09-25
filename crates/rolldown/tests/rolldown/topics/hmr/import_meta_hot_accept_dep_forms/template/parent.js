import { count } from './child.js';

globalThis.templateParentRuns = (globalThis.templateParentRuns ?? 0) + 1;
globalThis.templateSaw = count;

import.meta.hot.accept(`./child.js`, (mod) => {
  globalThis.templateSaw = mod.count;
});
