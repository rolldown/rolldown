// The first place of `x` is a call of `Object.defineProperty` with `configurable: true`. The
// bundler does not identify this form, so the order of `x` and `y` is not known. The CommonJS
// lexer of Node finds `x` only at the later assignment, which calls the setter.
let count = 0;
const g = {
  get x() {
    return ++count;
  },
};
Object.defineProperty(exports, 'x', {
  enumerable: true,
  configurable: true,
  get: function () {
    return g.x;
  },
  set(value) {},
});
exports.y = 0;
exports.x = 0;
Object.defineProperty(exports, 'y', {
  enumerable: true,
  get() {
    return ++count;
  },
});
globalThis.unknownCount = () => count;
