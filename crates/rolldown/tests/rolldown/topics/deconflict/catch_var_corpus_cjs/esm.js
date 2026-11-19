// An ESM module required by CJS: wrapped ESM.
import { x as other } from './other.js';

export var x = 'esm-x';
export const esmLog = [];
try {
  throw 0;
} catch (x) {
  var x = 'esm-c';
  esmLog.push(x, other);
}
esmLog.push(x);
export function innerEsm() {
  try {
    throw 0;
  } catch (x) {
    var x = 'inner';
    return [x, other];
  }
}
