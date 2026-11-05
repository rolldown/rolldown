import * as a from 'ext-ns';
import defer * as d from './d.js';
import defer * as d2 from './d2.js';

globalThis.__extLog.push('main');
export const same = () => [a === d.b, d.b === d2.c, a.y];
