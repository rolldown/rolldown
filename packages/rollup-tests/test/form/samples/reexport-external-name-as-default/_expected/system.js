System.register('bundle', ['external'], (function (exports) {
	'use strict';
	return {
		setters: [function (module) {
			exports('default', module.value);
		}],
		execute: (function () {



		})
	};
}));
