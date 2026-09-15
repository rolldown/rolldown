// Lazy routes, each using an adjacent pair of the staircase. The overlapping
// subsets ({common,c1}, {c1,c2}, {c2,c3}) give the shared modules distinct
// dependent-entry sets and thus distinct chunks.
import { common } from './common.js';
import { c1 } from './c1.js';
export default () => common + c1 + '/r1';
