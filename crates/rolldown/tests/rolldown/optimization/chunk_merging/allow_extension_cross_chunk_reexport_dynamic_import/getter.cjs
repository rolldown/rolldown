exports.boom = 0;
Object.defineProperty(exports, 'boom', {
  enumerable: true,
  get() {
    throw new Error('read of boom');
  },
});
