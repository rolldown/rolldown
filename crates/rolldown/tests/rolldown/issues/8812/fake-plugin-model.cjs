// IIFE accessing global set by fake-core. No exports.
// BUG: Without fix, this runs before fake-core sets globalThis.fakeCore.
(function () {
  'use strict';
  globalThis.fakeCore.register('dom-model');
})();
