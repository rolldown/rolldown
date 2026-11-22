exports.n = 0;
let reads = 0;
Object.defineProperty(exports, 'n', {
  enumerable: true,
  get() {
    reads += 1;
    return reads;
  },
});
