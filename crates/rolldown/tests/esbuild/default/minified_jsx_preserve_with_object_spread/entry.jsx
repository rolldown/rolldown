const obj = {
	before,
	...{ [key]: value },
	...{ key: value },
	after,
};
<Foo
	before
	{...{ [key]: value }}
	{...{ key: value }}
	after
/>;
<Bar
	{...{
		a,
		[b]: c,
		...d,
		e,
	}}
/>;