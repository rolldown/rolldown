// Under ESM output, the finalizer prints this CJS entry inside its CJS closure, so `Promise$2` is a
// root binding that the conflict resolver does not see. If the renamer renames `Promise` (a
// reserved name), the new name must skip the parameter `Promise$1` and also `Promise$2`.
const Promise = require('node:events');
const Promise$2 = 'sibling';
function param(Promise$1) {
  return Promise$1;
}
module.exports = [typeof Promise, Promise$2, param('nested')];
