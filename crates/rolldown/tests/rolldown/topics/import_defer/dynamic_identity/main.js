import { log } from './log.js';
import * as eager from './dep.js';
import defer * as deferred1 from './dep.js';
import defer * as deferred2 from './dep.js';
import { depDeferred as deferred3 } from './barrel.js';

const deferred4 = await import.defer('./dep.js');
export const result = [
  deferred1 === deferred2,
  deferred1 === deferred3,
  deferred1 === deferred4,
  deferred1 !== eager,
];
export { log };
