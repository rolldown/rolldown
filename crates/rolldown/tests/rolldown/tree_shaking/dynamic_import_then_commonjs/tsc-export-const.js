'use strict';
Object.defineProperty(exports, '__esModule', { value: true });
exports.then = void 0;
const then = function (resolve) {
  globalThis.thenCalled('tsc-export-const');
  resolve({});
};
exports.then = then;
