// Inferred-pure definer reached only through TWO star hops (outer-barrel -> inner-barrel -> here).
// The value is non-inlinable so a dropped `init_*` cannot be masked by constant folding.
function build() {
  return { value: 7 };
}

export const vDef = /* @__PURE__ */ build();
