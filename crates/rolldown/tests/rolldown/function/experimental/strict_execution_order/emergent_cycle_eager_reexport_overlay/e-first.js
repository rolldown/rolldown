// Side-effectful module hosted in the entry chunk: the predicted order runs it after the grouped
// chunks, the source order runs it before the definer subtree — the deviation that seeds the wrap
// plan with `definer` (imported after it through the forwarder) without touching `eagerhaz`.
(globalThis.__events ??= []).push('e-first');
export const first = true;
