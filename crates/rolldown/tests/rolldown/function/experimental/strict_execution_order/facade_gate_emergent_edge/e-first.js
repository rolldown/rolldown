// Side-effectful module in the root's own entry chunk: predicted order runs it after chunk A,
// source order before `wrapper` — the deviation that makes `wrapper` premature.
(globalThis.__events ??= []).push('e-first');
export const first = true;
