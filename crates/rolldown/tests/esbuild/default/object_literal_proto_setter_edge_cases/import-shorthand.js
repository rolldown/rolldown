import { __proto__, bar } from 'foo'
function foo() {
	console.log(
		'this must not become "{ __proto__: ... }":',
		{
			__proto__,
			bar,
		},
	)
}