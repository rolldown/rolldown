// Side-effectful module hosted in the first root's entry chunk: the predicted order runs it after
// the grouped chunks, the source order runs it before the wrapper subtree — the deviation that
// wraps the `wrapper` barrel without touching `eagerhaz`.
(globalThis.__events ??= []).push('e-first');
export const first = true;
