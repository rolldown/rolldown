// The CJS wrapper must not bind `arguments`: under CJS output, a top-level `arguments` resolves to
// the output file's own Node module wrapper, whose second argument is `require`.
module.exports = {
  sameThis: this === exports,
  sep: arguments[1]('node:path').sep,
};
