@x(() => 0) @y(() => 1)
export class c {
	fn() { return new c }
	static z = new c
}