exports.value = 0;
Object.defineProperty(exports, 'value', {
  enumerable: true,
  get() {
    console.log('getter');
    return 1;
  },
});
