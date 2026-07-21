// Earliest-discovered side-effectful member of chunk A: makes the entry import chunk A before
// chunk B, so at runtime A starts evaluating first and the lowering-added A -> B overlay import
// (the forwarder's `init_definer`) is what first reaches B.
(globalThis.__events ??= []).push('a-first');
export const aFirst = true;
