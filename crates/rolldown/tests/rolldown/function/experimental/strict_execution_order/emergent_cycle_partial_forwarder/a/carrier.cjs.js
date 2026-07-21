// CJS interop carrier in chunk A, body-assigned so an eager mid-cycle read from chunk B observes
// the not-yet-assigned wrapper var without the fixpoint.
module.exports = function carrier() {
  return 'CARRIED';
};
