// Earliest-discovered side-effectful member of chunk S: makes the entry import chunk S before
// chunk H, so at runtime S starts evaluating first and the lowering-added S -> H wrapper import
// (not the entry) is what first reaches H.
(globalThis.__events ??= []).push('s-first');
export const sFirst = true;
