@x(() => 0) @y(() => 1)
export default class f {
	fn() { return new f }
	static z = new f
}