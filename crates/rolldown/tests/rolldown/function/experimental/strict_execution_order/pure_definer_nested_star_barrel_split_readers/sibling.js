// Side-effectful sibling re-exported by the INNER barrel: it makes the inner barrel a
// non-transparent init owner, so an ancestor's retained-path traversal stops there and delegates
// the rest of the chain (the star hop to the pure definer) to the inner barrel's own `init_*`.
(globalThis.__events ??= []).push('sibling');

export const vSib = { value: 3 };
