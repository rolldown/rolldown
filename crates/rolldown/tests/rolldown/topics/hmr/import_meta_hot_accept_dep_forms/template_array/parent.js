import { count as a } from './a.js';
import { count as b } from './b.js';

globalThis.template_arrayParentRuns = (globalThis.template_arrayParentRuns ?? 0) + 1;
globalThis.template_arraySaw = [a, b];

import.meta.hot.accept([`./a.js`, './b.js'], ([newA, newB]) => {
  if (newA) globalThis.template_arraySaw[0] = newA.count;
  if (newB) globalThis.template_arraySaw[1] = newB.count;
});
