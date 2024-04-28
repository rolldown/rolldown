class KeepMe1 {
	static x = 'x'
	static y = 'y'
	static z = 'z'
}
class KeepMe2 {
	static x = 'x'
	static y = sideEffects()
	static z = 'z'
}
class KeepMe3 {
	static x = 'x'
	static y = 'y'
	static z = 'z'
}
new KeepMe3()