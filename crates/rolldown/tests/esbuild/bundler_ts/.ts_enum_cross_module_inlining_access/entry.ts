import {
	a_num, b_num, c_num, d_num, e_num,
	a_str, b_str, c_str, d_str, e_str,
} from './enums'

inlined = [
	a_num.x,
	b_num['x'],

	a_str.x,
	b_str['x'],
]

not_inlined = [
	c_num?.x,
	d_num?.['x'],
	e_num,

	c_str?.x,
	d_str?.['x'],
	e_str,
]