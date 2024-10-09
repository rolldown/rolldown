import { a, b, c, x } from './nested-constants'
console.log({
	'should be 4': ~(~a & ~b) & (b | c),
	'should be 32': ~(~x.a & ~x.b) & (x.b | x.c),
})