(function (global, factory) {
	typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
	typeof define === 'function' && define.amd ? define(['exports'], factory) :
	(global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global.computedProperties = {}));
})(this, (function (exports) { 'use strict';

	var foo = 'foo';
	var bar = 'bar';
	var baz = 'baz';
	var bam = 'bam';

	var x = { [foo]: 'bar' };

	class X {
		[bar] () {}
		get [baz] () {}
		set [bam] ( value ) {}
	}

	exports.X = X;
	exports.x = x;

}));
