let reads = 0;
// `a.cjs` assigns no value to `y`. Thus the output cannot tell which module of `barrel.js` gives
// `y`.
Object.defineProperty(exports, 'y', {
  enumerable: true,
  get() {
    reads += 1;
    return reads;
  },
});
exports.yReads = () => reads;
