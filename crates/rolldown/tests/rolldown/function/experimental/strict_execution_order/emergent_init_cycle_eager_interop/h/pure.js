// Side-effect-free definer in chunk H, order-wrapped through the premature deviation. It is the
// target of the barrel's re-export hop, so its `init_pure` is what the lowering imports across
// the chunk boundary, creating the emergent S -> H edge.
function makePv() {
  return 'PV';
}

export const pv = /* @__PURE__ */ makePv();
