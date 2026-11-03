// CJS interop carrier hosted in chunk A. Its wrapper declaration is body-assigned, so a phantom
// A <-> B cycle could make B read it before assignment (`require_* is not a function`). On-demand
// routing avoids that cycle; wrap-all defers the read.
module.exports = function carrier() {
  return 'CARRIED';
};
