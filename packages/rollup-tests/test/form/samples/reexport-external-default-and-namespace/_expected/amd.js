define(['exports', 'external'], (function (exports, external) { 'use strict';



	exports.default = external;
	Object.keys(external).forEach(function (k) {
		if (k !== 'default' && !exports.hasOwnProperty(k)) Object.defineProperty(exports, k, {
			enumerable: true,
			get: function () { return external[k]; }
		});
	});

	Object.defineProperty(exports, '__esModule', { value: true });

}));
