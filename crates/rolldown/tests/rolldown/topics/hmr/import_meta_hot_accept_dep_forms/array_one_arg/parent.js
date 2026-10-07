import './child.js';

globalThis.array_one_argParentRuns = (globalThis.array_one_argParentRuns ?? 0) + 1;

import.meta.hot.accept(['./child.js']);
