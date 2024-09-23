import * as js from './foo/no-side-effects.js'
import * as mjs from './foo/no-side-effects.mjs'
import * as cjs from './foo/no-side-effects.cjs'
console.log(
	js.default,
	mjs.default,
	cjs.default,
)