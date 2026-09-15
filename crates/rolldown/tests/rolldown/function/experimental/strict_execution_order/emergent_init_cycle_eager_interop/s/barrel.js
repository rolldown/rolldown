// Order-wrapped barrel in chunk S. Its re-export hop targets the wrapped `pure` in chunk H, so
// the lowering forwards `init_barrel` to `init_pure` — a *new* cross-chunk import S -> H that no
// pre-lowering analysis edge predicted. Together with the baseline H -> S edge (eagerhaz's CJS
// carrier) this closes the emergent chunk cycle.
export { pv } from '../h/pure.js';

function makeBmark() {
  return 'B';
}

// A pure call initializer: order-sensitive (so the deviation can flag this module) yet
// side-effect-free and not const-inlinable (so the binding stays materialized).
export const bmark = /* @__PURE__ */ makeBmark();
