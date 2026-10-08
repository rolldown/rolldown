exports.v = 1;
// Keeps `v` from being inlined as a constant.
exports.bump = () => {
  exports.v++;
};
