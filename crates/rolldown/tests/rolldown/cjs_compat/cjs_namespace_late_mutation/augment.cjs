const target = require('./target.cjs');

// Side-effect mutation: add a new property to another CommonJS module's
// exports object after it has been evaluated.
target.added = function added() {
  return 'added';
};
