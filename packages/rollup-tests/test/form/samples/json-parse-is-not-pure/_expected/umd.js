(function (global, factory) {
	typeof exports === 'object' && typeof module !== 'undefined' ? module.exports = factory() :
	typeof define === 'function' && define.amd ? define(factory) :
	(global = typeof globalThis !== 'undefined' ? globalThis : global || self, global.myBundle = factory());
})(this, (function () { 'use strict';

	var main = (input) => {
		try {
			JSON.parse(input);
			return true;
		} catch (e) {
			return false;
		}
	};

	return main;

}));
