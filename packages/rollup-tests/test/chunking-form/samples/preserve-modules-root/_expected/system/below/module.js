System.register(['./module2.js'], (function (exports) {
	'use strict';
	return {
		setters: [function (module) {
			exports('default', module.default);
		}],
		execute: (function () {



		})
	};
}));
