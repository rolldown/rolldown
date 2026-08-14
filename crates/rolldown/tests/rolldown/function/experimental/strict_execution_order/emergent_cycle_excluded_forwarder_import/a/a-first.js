// Earliest-discovered side-effectful member of chunk A: makes the first root import chunk A before
// chunk C, so at runtime A starts evaluating first and the lowering-added A -> C hop import is what
// first reaches C.
(globalThis.__events ??= []).push('a-first');
export const aFirst = true;
