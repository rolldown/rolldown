class REMOVE_ME {
	static x = 'x'
	static y = 'y'
	static z = 'z'
}
function REMOVE_ME_TOO() {
	new REMOVE_ME()
}
class KeepMe1 {
	static x = 'x'
	static y = sideEffects()
	static z = 'z'
}
class KeepMe2 {
	static x = 'x'
	static y = 'y'
	static z = 'z'
}
new KeepMe2()