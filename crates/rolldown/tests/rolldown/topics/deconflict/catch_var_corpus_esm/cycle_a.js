import { readA } from './cycle_b.js';

export var c = 'c0';
export const cycleLog = [];
try {
  throw 0;
} catch (c) {
  var c = 'c1';
  cycleLog.push(c, readA());
}
c = 'c2';
cycleLog.push(readA());
