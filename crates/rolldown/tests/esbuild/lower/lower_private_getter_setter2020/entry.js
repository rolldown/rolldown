export class Foo {
	get #foo() { return this.foo }
	set #bar(val) { this.bar = val }
	get #prop() { return this.prop }
	set #prop(val) { this.prop = val }
	foo(fn) {
		fn().#foo
		fn().#bar = 1
		fn().#prop
		fn().#prop = 2
	}
	unary(fn) {
		fn().#prop++;
		fn().#prop--;
		++fn().#prop;
		--fn().#prop;
	}
	binary(fn) {
		fn().#prop = 1;
		fn().#prop += 1;
		fn().#prop -= 1;
		fn().#prop *= 1;
		fn().#prop /= 1;
		fn().#prop %= 1;
		fn().#prop **= 1;
		fn().#prop <<= 1;
		fn().#prop >>= 1;
		fn().#prop >>>= 1;
		fn().#prop &= 1;
		fn().#prop |= 1;
		fn().#prop ^= 1;
		fn().#prop &&= 1;
		fn().#prop ||= 1;
		fn().#prop ??= 1;
	}
}