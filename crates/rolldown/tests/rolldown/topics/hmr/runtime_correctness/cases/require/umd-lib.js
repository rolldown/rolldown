(function(root, umdLib) {
  if (typeof require === "function" && typeof exports === "object" && typeof module === "object") {
    module.exports = umdLib();
  } else if (typeof define === "function" && define.amd) {
    define(function() {
      return umdLib();
    });
  } else {
    root.umdLib = umdLib();
  }
})(this, function() {
  const exports = function () {
    return 'exports'
  }
  exports.foo = 'foo'
  return exports
})
