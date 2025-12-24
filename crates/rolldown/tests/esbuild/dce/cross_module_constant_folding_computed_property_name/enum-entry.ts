import { x } from './enum-constants'
console.log({
	[x.a]: x.a,
	[x.b]: x.b,
})
class Foo {
	[x.proto] = {};
	[x.ptype] = {};
	[x.ctor]() {};
}