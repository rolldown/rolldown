// `process` is also an unresolved global referenced by entry-b. Duplicating this
// leaf would make the copied `const process` shadow that global, so the pass must
// keep util.js as a standalone shared chunk instead of duplicating it.
export const process = 42;
export function getProc() {
  return process;
}
