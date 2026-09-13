// The `exports` parameter forces the alias, and both names the alias would prefer are already
// source bindings, so it must land on `exports_lib_alias$2`. Direct eval still resolves all three
// source names.
var exports_lib_alias = { answer: 0 };
this.answer = 42;
const read = (exports, exports_lib_alias$1) =>
  eval('exports.answer') + eval('exports_lib_alias$1.answer') + eval('exports_lib_alias.answer');
module.exports = {
  answer: read({ answer: 0 }, { answer: 0 }) === 0 ? this.answer : -1,
};
