exports.foo = 1;

function createExports() {
  return { foo: 2 };
}

// Non-object-literal RHS — all exports.xxx constants should be invalidated
// since we can't statically determine the property names.
module.exports = createExports();
