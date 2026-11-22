// The first place of `x` comes before the first place of `y`, so Node reads `x` first.
exports.x = 0;
exports.y = 0;
let count = 0;
Object.defineProperty(exports, 'x', {
  enumerable: true,
  get() {
    return ++count;
  },
});
Object.defineProperty(exports, 'y', {
  enumerable: true,
  get() {
    return ++count;
  },
});
globalThis.xyCount = () => count;
