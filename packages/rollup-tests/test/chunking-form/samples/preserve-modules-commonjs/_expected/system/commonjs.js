System.register(['external', './other.js', './_virtual/other.js'], (function (exports) {
	'use strict';
	var require$$0, other;
	return {
		setters: [function (module) {
			require$$0 = module.default;
		}, null, function (module) {
			other = module.__exports;
		}],
		execute: (function () {

			const external = require$$0;
			const { value } = other;

			console.log(external, value);

			var commonjs = exports('default', 42);

		})
	};
}));
