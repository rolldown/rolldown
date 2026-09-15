// Defines `eg` — the binding lazyBarrel prunes while account.js still references
// it. Side-effect-free initializer (object spreads), so eligible for removal once
// its import record is dropped.
const primitives = { string: { tag: 's' }, number: { tag: 'n' } };
const higher = { object: (shape) => ({ tag: 'o', shape, parse: () => true }) };
export const eg = { ...primitives, ...higher };
