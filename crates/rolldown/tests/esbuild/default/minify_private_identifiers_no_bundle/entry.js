class Foo {
	#foo
	foo = class {
		#foo
		#foo2
		#bar
	}
	get #bar() {}
	set #bar(x) {}
}
class Bar {
	#foo
	foo = class {
		#foo2
		#foo
		#bar
	}
	get #bar() {}
	set #bar(x) {}
}