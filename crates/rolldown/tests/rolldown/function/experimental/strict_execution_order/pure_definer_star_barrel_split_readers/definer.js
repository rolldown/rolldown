// Inferred-pure definer: the top level is only pure statements, so tree-shaking judges this module
// side-effect-free by inference (no `sideEffects` metadata involved). The exported value is
// non-inlinable — an object built by a local function behind a PURE annotation — so a dropped
// `init_*` cannot be masked by constant folding; `vDef` stays `undefined` until the init runs.
function build() {
  return { value: 7 };
}

export const vDef = /* @__PURE__ */ build();
