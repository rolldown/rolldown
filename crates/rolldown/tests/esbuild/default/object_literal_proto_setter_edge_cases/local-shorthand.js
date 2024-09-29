function foo(__proto__, bar) {
	{
		let __proto__, bar // These locals will be renamed
		console.log(
			'this must not become "{ __proto__: ... }":',
			{
				__proto__,
				bar,
			},
		)
	}
}