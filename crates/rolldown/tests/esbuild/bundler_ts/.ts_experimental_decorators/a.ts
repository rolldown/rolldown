@x(() => 0) @y(() => 1)
class a_class {
	fn() { return new a_class }
	static z = new a_class
}
export let a = a_class