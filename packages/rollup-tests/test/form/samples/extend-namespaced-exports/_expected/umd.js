(function (global, factory) {
	typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
	typeof define === 'function' && define.amd ? define(['exports'], factory) :
	(global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory((global.foo = global.foo || {}, global.foo.bar = global.foo.bar || {}, global.foo.bar.baz = global.foo.bar.baz || {})));
})(this, (function (exports) { 'use strict';

	const answer = 42;

	exports.answer = answer;

}));
