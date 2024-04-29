function dec(x: any): any {}
export function fn(x: string): any {
	class Foo {
		@dec(arguments[0])
		[arguments[0]]() {}
	}
	return Foo;
}