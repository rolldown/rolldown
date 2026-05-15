// Regression for #9263: CJS-classified module + class field + ES2021 target
// would lower `count = 0` to `_defineProperty(this, "count", 0)` and emit
// `var _defineProperty = (init_defineProperty(), __toCommonJS(defineProperty_exports))`,
// binding `_defineProperty` to the helper module's namespace object instead of
// the helper function — throwing `TypeError: _defineProperty is not a function`
// at init time, before any user code runs.
class Counter {
  count = 0;
  tick() {
    this.count += 1;
  }
}

const c = new Counter();
c.tick();
if (c.count !== 1) {
  throw new Error('class field init failed');
}
