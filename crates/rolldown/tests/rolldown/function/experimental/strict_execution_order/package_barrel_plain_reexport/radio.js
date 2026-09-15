// Second side-effect-free component definer plain-imported and re-exported by the same barrel.
function buildComponent() {
  return { name: 'Radio' };
}

const Radio = /* @__PURE__ */ buildComponent();

export default Radio;
