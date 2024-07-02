class Foo {
	@dec(1) prop1() {}
	@dec(2) prop2_() {}
	@dec(3) ['prop3']() {}
	@dec(4) ['prop4_']() {}
	@dec(5) [/* @__KEY__ */ 'prop5']() {}
	@dec(6) [/* @__KEY__ */ 'prop6_']() {}
}