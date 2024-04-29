let tests = [
	{...a, ...b},
	{a, b, ...c},
	{...a, b, c},
	{a, ...b, c},
	{a, b, ...c, ...d, e, f, ...g, ...h, i, j},
]
let jsx = [
	<div {...a} {...b}/>,
	<div a b {...c}/>,
	<div {...a} b c/>,
	<div a {...b} c/>,
	<div a b {...c} {...d} e f {...g} {...h} i j/>,
]