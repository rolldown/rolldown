import { X } from './x.js';
// Read while the module evaluates: the chunk holding `x.js` must run first.
const prefix = 'y' + X.value;
export function Y() {
  return prefix;
}
