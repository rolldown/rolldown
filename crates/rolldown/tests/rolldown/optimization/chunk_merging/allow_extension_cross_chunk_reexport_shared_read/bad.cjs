exports.x = 0;
let calls = 0;
Object.defineProperty(exports, 'x', {
  enumerable: true,
  get() {
    calls += 1;
    throw new Error('read of x');
  },
});
exports.calls = () => calls;
