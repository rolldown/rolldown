import { a, b, proto, ptype, ctor } from './const-constants'
console.log({
	[a]: a,
	[b]: b,
})
class Foo {
	[proto] = {};
	[ptype] = {};
	[ctor]() {};
}