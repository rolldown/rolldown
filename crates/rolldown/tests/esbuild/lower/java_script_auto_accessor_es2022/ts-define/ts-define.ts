class Foo {
	accessor one = 1
	accessor #two = 2
	accessor [three()] = 3

	static accessor four = 4
	static accessor #five = 5
	static accessor [six()] = 6
}
class Normal { accessor a = b; c = d }
class Private { accessor #a = b; c = d }
class StaticNormal { static accessor a = b; static c = d }
class StaticPrivate { static accessor #a = b; static c = d }