System.register([], (function (exports, module) {
	'use strict';
	return {
		execute: (function () {

			module.import('./foo.js').then(result => console.log(result));

		})
	};
}));
