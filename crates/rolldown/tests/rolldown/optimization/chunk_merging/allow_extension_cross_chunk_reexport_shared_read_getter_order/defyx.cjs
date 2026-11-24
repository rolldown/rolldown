// The CommonJS lexer of Node finds `y` in the call of `Object.defineProperty`, because the getter
// only returns `g.y`. The call comes before the first place of `x`, so Node reads `y` first.
let count = 0;
const g = {
  get y() {
    return ++count;
  },
};
Object.defineProperty(exports, 'y', {
  enumerable: true,
  get: function () {
    return g.y;
  },
});
exports.x = 0;
Object.defineProperty(exports, 'x', {
  enumerable: true,
  get() {
    return ++count;
  },
});
globalThis.defyxCount = () => count;
