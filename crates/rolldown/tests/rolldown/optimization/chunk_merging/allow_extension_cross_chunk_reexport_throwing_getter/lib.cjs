exports.x = 37;
Object.defineProperty(exports, 'x', {
  enumerable: true,
  get() {
    throw new Error('read of x');
  },
});
