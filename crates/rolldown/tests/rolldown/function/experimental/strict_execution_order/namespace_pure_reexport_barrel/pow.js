// Second side-effect-free definer sibling re-exported by the same barrel.
function makePow() {
  return { value: 3 };
}

var unit = /* @__PURE__ */ makePow();

export function scalePow() {
  return unit;
}
