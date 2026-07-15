// Side-effect-free definer: a hoisted factory function plus a module-level `unit` assigned from a
// pure call at init time (mirrors d3-scale's `var unit` / `class InternMap`). Tree-shaking judges
// this module side-effect-free, so it is not an execution dependency of the barrel that re-exports
// it. The `unit` binding is only assigned when `init_linear()` runs.
function makeUnit() {
  return { value: 7 };
}

var unit = /* @__PURE__ */ makeUnit();

export function scaleLinear() {
  return unit;
}
