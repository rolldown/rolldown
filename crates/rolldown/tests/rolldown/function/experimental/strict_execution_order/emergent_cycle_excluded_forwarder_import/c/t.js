// Definer in chunk C, order-wrapped through the second root's premature deviation. `tv` is consumed
// by the second root (keeps `t` order-sensitive and included); `unused` is imported by the
// non-included forwarder `f` but never re-exported, so only the excluded-statement metadata's
// walk-every-static-import routing reaches it — the divergence the projector misses.
function mkTv() {
  return 'TV';
}

export const tv = /* @__PURE__ */ mkTv();
export const unused = 'UNUSED';
