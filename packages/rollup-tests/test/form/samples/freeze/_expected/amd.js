define(['exports'], (function (exports) { 'use strict';

	const foo = 1;
	const bar = 2;

	var namespace = {
		__proto__: null,
		bar: bar,
		foo: foo
	};

	console.log( Object.keys( namespace ) );

	const a = 1;
	const b = 2;

	exports.a = a;
	exports.b = b;

}));
