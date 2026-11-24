// Node reads `z`, then `x`.
exports.z = 0;
exports.x = 0;
let count = 0;
Object.defineProperty(exports, 'x', {
  enumerable: true,
  get() {
    return ++count;
  },
});
Object.defineProperty(exports, 'z', {
  enumerable: true,
  get() {
    return ++count;
  },
});
