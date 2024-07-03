const keepThisToo = Symbol('keepThisToo')
declare const REMOVE_THIS_TOO: unique symbol
abstract class Foo {
	keepThis: any
	[keepThisToo]: any
	abstract REMOVE_THIS: any
	abstract [REMOVE_THIS_TOO]: any
	abstract [(x => y => x + y)('nested')('scopes')]: any
}
(() => new Foo())()