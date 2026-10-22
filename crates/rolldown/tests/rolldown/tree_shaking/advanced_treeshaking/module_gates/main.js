import { value } from './tla.js';
import { readEval } from './eval.js';

// The analysis covers a module with top-level await.
await 1;
const o = { used: 1, dead: 'DEAD_tla' };
export function main() {
  return o.used + value + readEval();
}
