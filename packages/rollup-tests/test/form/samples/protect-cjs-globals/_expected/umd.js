(function (global, factory) {
	typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
	typeof define === 'function' && define.amd ? define(['exports'], factory) :
	(global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global.bundle = {}));
})(this, (function (exports) { 'use strict';

	const exports$1 = 1;
	const require = 2;
	const module = 3;
	const __filename = 4;
	const __dirname = 5;

	exports.__dirname = __dirname;
	exports.__filename = __filename;
	exports.exports = exports$1;
	exports.module = module;
	exports.require = require;

}));
