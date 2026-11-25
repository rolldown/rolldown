exports.then = 0;
exports.foo = 0;
Reflect.defineProperty(exports, 'foo', {
  get() {
    exports.then = (resolve) => resolve('assimilated');
    return 42;
  },
});
