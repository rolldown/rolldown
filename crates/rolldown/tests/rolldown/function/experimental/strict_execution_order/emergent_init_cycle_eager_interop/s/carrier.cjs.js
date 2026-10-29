// CJS interop carrier hosted in chunk S. Its `var require_carrier_cjs = __commonJSMin(...)`
// declaration is assigned in S's chunk *body* — during the emergent cycle's evaluation, chunk H's
// body runs before S's, so an eager read of this wrapper from H observes the unassigned var
// (vue-vben-admin's `qe is not a function` shape).
module.exports = function carrier() {
  return 'CARRIED';
};
