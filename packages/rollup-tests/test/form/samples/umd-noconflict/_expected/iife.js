var FooBar = (function (exports) {
	'use strict';

	function doThings() {
		console.log( 'doing things...' );
	}

	const number = 42;

	var setting = 'no';

	exports.doThings = doThings;
	exports.number = number;
	exports.setting = setting;

	return exports;

})({});
