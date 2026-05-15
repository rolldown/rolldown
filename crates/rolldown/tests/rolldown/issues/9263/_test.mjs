// Regression assertion: prior to the fix, ES2021 lowering of the `count = 0`
// class field produced `_defineProperty(this, "count", 0)`, but the wrap path
// for CJS-classified modules emitted
//   var _defineProperty = (init_defineProperty(), __toCommonJS(defineProperty_exports));
// — binding the local to the helper module's namespace `{ __esModule: true, default: <fn> }`
// instead of the helper function. Calling `_defineProperty(...)` threw
// `TypeError: _defineProperty is not a function` at module-init time.
// After the fix, the wrap appends `.default`, so the local is the function and init succeeds.
await import('./dist/main.js');
