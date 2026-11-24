// Node reads `x`, then `y`.
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
