// CJS interop carrier hosted in chunk A. Its `var require_carrier = __commonJSMin(...)`
// declaration is assigned in A's chunk *body* — during the emergent cycle's evaluation, chunk B's
// body runs before A's, so an eager read of this wrapper from B observes the unassigned var
// (vue-vben-admin's `qe is not a function` shape).
module.exports = function carrier() {
  return 'CARRIED';
};
