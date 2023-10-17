const assert = require('node:assert');

module.exports = {
	description: 'calls to externally reassigned methods of named reexports must be retained',
	exports(exports) {
		let triggered = false;
		exports.obj.reassigned = function () {
			triggered = true;
		};
		exports.test();
		assert.ok(triggered);
	}
};
