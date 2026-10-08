import { loadOther } from './other.js';

export function load(flag) {
  return flag ? import('./a.js') : import('./b.js');
}

export { loadOther };
