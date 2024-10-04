let { esm_foo_ } = require('./esm')
let { cjs_foo_ } = require('./cjs')
exports.bar_ = [
	esm_foo_,
	cjs_foo_,
]