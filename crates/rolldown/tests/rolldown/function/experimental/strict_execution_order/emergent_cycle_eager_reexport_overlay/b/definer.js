// Side-effect-free definer in chunk B, order-wrapped through the premature deviation. It is the
// target of the forwarder's re-export hop, so its `init_definer` is what the lowering imports
// across the chunk boundary via the eager forwarder's overlay, creating the emergent A -> B edge.
function makePv() {
  return 'PV';
}

// A pure call initializer: order-sensitive (so the deviation can flag this module) yet
// side-effect-free and not const-inlinable (so the binding stays materialized).
export const pv = /* @__PURE__ */ makePv();
