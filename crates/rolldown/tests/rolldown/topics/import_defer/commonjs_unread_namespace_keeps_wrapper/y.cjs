// `x.cjs` declares the same top-level name.
function snapshot() {
  return 'y';
}
globalThis.__events.push(snapshot());
exports.foo = snapshot;
