// Requesting this re-export is what makes the shared `inner.js` record
// "occupied" and triggers the skip of the locally-used `setup`.
export function helper() {
  return 'helper';
}
