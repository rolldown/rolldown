System.register(['./generated-shared.js'], (function (exports) {
	'use strict';
	var data;
	return {
		setters: [function (module) {
			data = module.d;
		}],
		execute: (function () {

			var main2 = exports('default', data.map(d => d + 2));

		})
	};
}));
