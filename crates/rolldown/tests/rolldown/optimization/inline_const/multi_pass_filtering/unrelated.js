// This module doesn't import any constants
// With the optimization, it should be skipped in pass 2+
export const UNRELATED = 'hello';
export function unrelatedFn() {
  return UNRELATED;
}
