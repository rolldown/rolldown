import { a, b, c, x } from './nested-constants'
console.log({
	'should be foobarbaz': a + b + c,
	'should be FOOBARBAZ': x.a + x.b + x.c,
})