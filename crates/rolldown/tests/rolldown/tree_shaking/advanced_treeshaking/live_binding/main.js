import { count, inc } from './counter.js';
export function read() {
  return count === 0 ? 'zero' : 'more';
}
export { inc };
