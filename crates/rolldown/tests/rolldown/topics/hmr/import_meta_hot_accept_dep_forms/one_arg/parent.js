import './child.js';

globalThis.one_argParentRuns = (globalThis.one_argParentRuns ?? 0) + 1;

import.meta.hot.accept('./child.js');
