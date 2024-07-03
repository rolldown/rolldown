@x(() => 0) @y(() => 1)
abstract class b_class {
	fn() { return new b_class }
	static z = new b_class
}
export let b = b_class