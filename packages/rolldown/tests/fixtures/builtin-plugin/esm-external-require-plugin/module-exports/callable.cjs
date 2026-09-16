function callable(value) {
  return value + 1;
}

callable.kind = 'commonjs';
module.exports = callable;
