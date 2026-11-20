import { a } from './barrel.js';
import defer * as ns from './empty.js';

let result;
try {
  result = [a.value, typeof ns.f];
} catch (e) {
  result = e.name;
}

export function read() {
  return result;
}
