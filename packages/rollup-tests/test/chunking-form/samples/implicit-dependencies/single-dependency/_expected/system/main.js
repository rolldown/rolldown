System.register([], (function (exports) {
	'use strict';
	return {
		execute: (function () {

			const value = exports('v', 42);

			console.log(value);

		})
	};
}));
