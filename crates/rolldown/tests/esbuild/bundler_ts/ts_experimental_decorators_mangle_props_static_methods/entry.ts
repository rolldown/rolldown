class Foo {
	@dec(1) static prop1() {}
	@dec(2) static prop2_() {}
	@dec(3) static ['prop3']() {}
	@dec(4) static ['prop4_']() {}
	@dec(5) static [/* @__KEY__ */ 'prop5']() {}
	@dec(6) static [/* @__KEY__ */ 'prop6_']() {}
}