module.exports = function run() {
  const values = [];
  try {
    throw 0;
  } catch (require) {
    var require = 1;
    {
      function require() {
        return 2;
      }
      values.push(require());
    }
    values.push(typeof require);
  }
  return [values, typeof require];
};
