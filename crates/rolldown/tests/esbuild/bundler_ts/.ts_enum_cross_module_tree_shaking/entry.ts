import {
	a_DROP,
	b_DROP,
	c_DROP,
} from './enums'

console.log([
	a_DROP.x,
	b_DROP['x'],
	c_DROP.x,
])

import {
	a_keep,
	b_keep,
	c_keep,
	d_keep,
	e_keep,
} from './enums'

console.log([
	a_keep.x,
	b_keep.x,
	c_keep,
	d_keep.y,
	e_keep.x,
])