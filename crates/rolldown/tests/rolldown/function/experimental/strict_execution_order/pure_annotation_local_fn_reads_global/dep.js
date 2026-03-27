// `getGlobalValue` is a local function whose body accesses a global variable.
// The `/*#__PURE__*/` annotation means the call itself is not treated as
// having side effects (`Unknown`), and because the callee is a local
// identifier (not a global like `Reflect.something`), there is no
// `GlobalVarAccess` flag on this statement either.
//
// Without the `PureAnnotation` flag in the `ExecutionOrderSensitive` check,
// this module would NOT be wrapped, and `getGlobalValue()` would execute
// before `setup.js` has a chance to set `globalThis.globalValue`, producing
// `undefined` instead of `'foo'`.
function getGlobalValue() {
  return globalValue;
}

export default /*#__PURE__*/ getGlobalValue();
