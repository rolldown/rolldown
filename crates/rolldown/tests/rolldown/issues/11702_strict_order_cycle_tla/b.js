// `b` is pulled into the cycle back-edge and has no `await` of its own either.
// It must not call back into `a`: that would recurse at runtime and mask the
// init-order result this fixture is asserting.
import { a } from './a.js';

export const bValue = 2;

export function b() {
  return bValue;
}
