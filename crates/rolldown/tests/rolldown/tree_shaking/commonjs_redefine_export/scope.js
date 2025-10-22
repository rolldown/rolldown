"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.clearScope = clearScope;
exports.scope = exports.path = void 0;
exports.scope = new WeakMap();
function clearScope() {
  exports.scope = scope = new WeakMap();
}
