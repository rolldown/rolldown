// CJS IIFE: sets globalThis.fakeCore, exports via module.exports.
(function () {
  'use strict';
  var fakeCore = {
    registry: [],
    register: function (name) {
      this.registry.push(name);
    },
  };
  globalThis.fakeCore = fakeCore;
  if (typeof module === 'object') {
    try {
      module.exports = fakeCore;
    } catch (_e) {}
  }
})();
