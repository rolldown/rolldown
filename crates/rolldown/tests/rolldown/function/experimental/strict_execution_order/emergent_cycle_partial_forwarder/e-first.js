// Entry-chunk side effect: the predicted order runs it after the grouped chunks, the source order
// before the forwarder subtree — the deviation that seeds the wrap plan.
(globalThis.__events ??= []).push('e-first');
export const first = true;
