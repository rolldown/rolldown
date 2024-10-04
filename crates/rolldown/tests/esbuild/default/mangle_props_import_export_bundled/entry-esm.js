import { esm_foo_ } from './esm'
import { cjs_foo_ } from './cjs'
import * as esm from './esm'
import * as cjs from './cjs'
export let bar_ = [
	esm_foo_,
	cjs_foo_,
	esm.esm_foo_,
	cjs.cjs_foo_,
]