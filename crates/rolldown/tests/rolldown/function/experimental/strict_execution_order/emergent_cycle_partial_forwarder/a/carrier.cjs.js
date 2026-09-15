// CJS interop carrier in chunk A. A phantom reverse edge could expose its body-assigned wrapper to
// an eager mid-cycle read from B; on-demand routing avoids that edge and wrap-all defers the read.
module.exports = function carrier() {
  return 'CARRIED';
};
