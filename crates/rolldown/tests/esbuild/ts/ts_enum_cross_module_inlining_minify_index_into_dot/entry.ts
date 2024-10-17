const enum Foo {
	foo1 = 'abc',
	foo2 = 'a b c',
}
import { Bar } from './lib'
inlined = [
	obj[Foo.foo1],
	obj[Bar.bar1],
	obj?.[Foo.foo1],
	obj?.[Bar.bar1],
	obj?.prop[Foo.foo1],
	obj?.prop[Bar.bar1],
]
notInlined = [
	obj[Foo.foo2],
	obj[Bar.bar2],
	obj?.[Foo.foo2],
	obj?.[Bar.bar2],
	obj?.prop[Foo.foo2],
	obj?.prop[Bar.bar2],
]