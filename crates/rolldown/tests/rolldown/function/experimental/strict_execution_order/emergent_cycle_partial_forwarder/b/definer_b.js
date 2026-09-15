// Second order-wrapped definer in chunk B. `bv` is consumed directly by the entry (so definer_b is
// included and order-wrapped), while `unused` is only re-exported by the forwarder and consumed by
// nobody — the forwarder's excluded hop targets it.
function makeBv() {
  return 'BV';
}
export const bv = /* @__PURE__ */ makeBv();
export const unused = 'UNUSED';
