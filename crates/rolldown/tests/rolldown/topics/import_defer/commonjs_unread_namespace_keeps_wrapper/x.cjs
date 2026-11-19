// `y.cjs` declares the same top-level name.
function snapshot() {
  return 'x';
}
globalThis.__events.push(snapshot());
exports.foo = snapshot;
