export function before() {
	console.log(Foo.FOO)
}
enum Foo { FOO }
export function after() {
	console.log(Foo.FOO)
}