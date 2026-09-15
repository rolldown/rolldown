// Side-effect-free definer in chunk B, order-wrapped through the premature deviation. It is the
// target of the forwarder's re-export hop: on-demand routing imports its initializer at the real
// consumer, while wrap-all may reach it through the forwarder's wrapper.
function makePv() {
  return 'PV';
}

// A pure call initializer: order-sensitive (so the deviation can flag this module) yet
// side-effect-free and not const-inlinable (so the binding stays materialized).
export const pv = /* @__PURE__ */ makePv();
