// The CommonJS lexer of Node finds `x` in the call of `Object.defineProperty`, because the getter
// only returns `g.x`. The call comes before the first place of `y`, so Node reads `x` first.
let count = 0;
const g = {
  get x() {
    return ++count;
  },
};
Object.defineProperty(exports, 'x', {
  enumerable: true,
  get: function () {
    return g.x;
  },
});
exports.y = 0;
Object.defineProperty(exports, 'y', {
  enumerable: true,
  get() {
    return ++count;
  },
});
globalThis.defxyCount = () => count;
