// Second pure definer: consumed ONLY through the opaque namespace object (no statically resolved
// read records its chain). A path-restricted barrel walk would drop its init even though the
// included namespace retains its binding.
function build() {
  return { value: 11 };
}

export const wDef = /* @__PURE__ */ build();
