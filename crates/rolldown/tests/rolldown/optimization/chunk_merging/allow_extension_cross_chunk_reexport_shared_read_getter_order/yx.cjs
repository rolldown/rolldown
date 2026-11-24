// The first place of `y` comes before the first place of `x`, so Node reads `y` first. The later
// assignment of `y` and the order of the getters do not change this order.
exports.y = 0;
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
globalThis.yxCount = () => count;
