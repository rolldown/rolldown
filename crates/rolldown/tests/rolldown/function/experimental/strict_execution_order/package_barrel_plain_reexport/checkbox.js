// Side-effect-free component definer (mirrors a `@carbon/react` `forwardRef` component): a pure call
// builds the component, assigned to a module-level binding at init time. Only assigned when
// `init_checkbox()` runs.
function buildComponent() {
  return { name: 'Checkbox' };
}

const Checkbox = /* @__PURE__ */ buildComponent();

export default Checkbox;
