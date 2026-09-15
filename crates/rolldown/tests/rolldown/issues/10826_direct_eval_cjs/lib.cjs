var modules = {
  './a.js': function (module, exports, __webpack_require__) {
    eval('exports.answer = 42;');
  },
};
function load(id) {
  var module = { exports: {} };
  modules[id].call(module.exports, module, module.exports, load);
  return module.exports;
}
module.exports = { answer: load('./a.js').answer };
