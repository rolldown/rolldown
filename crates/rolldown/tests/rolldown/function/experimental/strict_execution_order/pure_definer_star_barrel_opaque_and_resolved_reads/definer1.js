// First pure definer: consumed through a statically RESOLVED namespace member read, so its chain
// is on the barrel record's recorded retained path.
function build() {
  return { value: 7 };
}

export const vDef = /* @__PURE__ */ build();
