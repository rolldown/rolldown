const keepThis = Symbol('keepThis')
declare const AND_REMOVE_THIS: unique symbol
abstract class Foo {
	REMOVE_THIS: any
	[keepThis]: any
	abstract REMOVE_THIS_TOO: any
	abstract [AND_REMOVE_THIS]: any
	abstract [(x => y => x + y)('nested')('scopes')]: any
}
(() => new Foo())()