enum a_num { x = 123 }
enum b_num { x = 123 }
enum c_num { x = 123 }
enum d_num { x = 123 }
enum e_num { x = 123 }

enum a_str { x = 'abc' }
enum b_str { x = 'abc' }
enum c_str { x = 'abc' }
enum d_str { x = 'abc' }
enum e_str { x = 'abc' }

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