(function (global, factory) {
	typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
	typeof define === 'function' && define.amd ? define(['exports'], factory) :
	(global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global.myModule = {}));
})(this, (function (exports) { 'use strict';

	exports.Foo = class Foo {};
	exports.Foo = lol( exports.Foo );

}));
