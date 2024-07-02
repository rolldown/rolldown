class Foo {
	@dec(1) static prop1 = null
	@dec(2) static prop2_ = null
	@dec(3) static ['prop3'] = null
	@dec(4) static ['prop4_'] = null
	@dec(5) static [/* @__KEY__ */ 'prop5'] = null
	@dec(6) static [/* @__KEY__ */ 'prop6_'] = null
}