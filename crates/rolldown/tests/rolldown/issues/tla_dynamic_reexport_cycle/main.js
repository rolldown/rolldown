globalThis.__compatLog ??= [];
import { v0 as rootValue0 } from './d0/m0.js';
import * as rootNs1 from './d0/m1.js';
import rootDefault2 from './d0/m2.js';
const dynamicRoot = await import('./d0/m2.js');
export const result =
  [rootValue0, rootNs1.v1, rootDefault2, dynamicRoot.v2].join('|') + ':339772549';
export const sideEffects = globalThis.__compatLog.join(',');
