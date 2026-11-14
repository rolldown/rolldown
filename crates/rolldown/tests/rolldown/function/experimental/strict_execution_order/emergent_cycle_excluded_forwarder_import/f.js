// Non-included forwarder. Nothing consumes its exports (the barrel's `export * from` hop is
// tree-shaken), so `f` itself is dropped from the output — but its `import { unused } from` record
// still resolves to the order-wrapped `t`. The excluded-statement metadata walks every static
// import of a non-included forwarder, so it registers `init_t` for the barrel that re-exports `f`;
// the projector, which only follows `f`'s resolved *exports*, never reaches `t` because `unused` is
// not re-exported.
import { unused } from './c/t.js';

export const forwarded = unused;
