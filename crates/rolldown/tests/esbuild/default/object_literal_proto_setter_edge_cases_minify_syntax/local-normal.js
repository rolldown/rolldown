function foo(__proto__, bar) {
	console.log(
		'this must not become "{ __proto__ }":',
		{
			__proto__: __proto__,
			bar: bar,
		},
	)
}