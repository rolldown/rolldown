// CJS-classified module with a class field — under target es2021 this
// lowers `count = 0` to a synthesized `_defineProperty(this, "count", 0)`
// require, which is the exact site #9263 fixes.
class Counter {
  count = 0;
  tick() {
    this.count += 1;
  }
}

module.exports = Counter;
