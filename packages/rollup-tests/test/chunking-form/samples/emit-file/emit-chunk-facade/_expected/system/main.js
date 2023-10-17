System.register(['./generated-build-starter.js'], (function () {
	'use strict';
	var value, otherValue;
	return {
		setters: [function (module) {
			value = module.v;
			otherValue = module.o;
		}],
		execute: (function () {

			console.log('main', value, otherValue);

		})
	};
}));
