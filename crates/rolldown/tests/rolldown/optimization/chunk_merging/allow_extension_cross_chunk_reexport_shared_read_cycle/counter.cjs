// The getters give constant values.
exports.x = 0;
exports.z = 0;
Object.defineProperty(exports, 'x', {
  enumerable: true,
  get() {
    return 'x';
  },
});
Object.defineProperty(exports, 'z', {
  enumerable: true,
  get() {
    return 'z';
  },
});
