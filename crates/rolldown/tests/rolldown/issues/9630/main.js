// Entry chunk. Two things force the repro:
//   1. eagerly import `isArrayLike` -> its wrapper is hoisted into the ENTRY chunk.
//   2. lazily import `last`        -> it lands in a SEPARATE (lazy) chunk that must
//      reach back across the boundary for the leaf's wrapper.
import { isArrayLike } from './isArrayLike.cjs';

export const eager = isArrayLike([1]);

// Evaluating the lazy chunk runs `require_last()`, which is where the bug bit:
// `TypeError: require_isArrayLike is not a function`.
export async function lazy() {
  const last = await import('./last.cjs');
  return last.default.last([1, 2, 3]);
}
