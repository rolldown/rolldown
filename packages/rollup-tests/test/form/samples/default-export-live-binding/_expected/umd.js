(function (global, factory) {
	typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
	typeof define === 'function' && define.amd ? define(['exports'], factory) :
	(global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global.bundle = {}));
})(this, (function (exports) { 'use strict';

	exports.default = null;
	const setFoo = value => (exports.default = value);

	exports.setFoo = setFoo;

	Object.defineProperty(exports, '__esModule', { value: true });

}));
