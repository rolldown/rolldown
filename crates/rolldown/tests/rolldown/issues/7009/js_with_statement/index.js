var obj = { a: 1, b: 2 };
with (obj) {
  exports.result = a + b;
}
