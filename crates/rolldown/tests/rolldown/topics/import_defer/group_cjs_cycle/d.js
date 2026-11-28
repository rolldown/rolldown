import { observed, same } from './p.js';

export function read() {
  return [observed, same()];
}
