// CJS-classified module exercising multiple oxc-runtime helpers under
// target es2021:
//   - `count = 0`              → `_defineProperty`
//   - `#priv = 1`              → `_classPrivateFieldInitSpec`
//   - `return this.#priv`      → `_classPrivateFieldGet`
// Every synthesized helper require must resolve to the callable, not the
// namespace — i.e. the boundary covers all helpers, not only the one in
// the original #9263 report.
class Counter {
  count = 0;
  #priv = 1;

  bump() {
    this.count += 1;
  }

  getPriv() {
    return this.#priv;
  }
}

const c = new Counter();
c.bump();
if (c.count !== 1) {
  throw new Error('class field path failed');
}
if (c.getPriv() !== 1) {
  throw new Error('private field path failed');
}
