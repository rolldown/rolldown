class A {
	static {}
	static {
		this.thisField++
		A.classField++
		super.superField = super.superField + 1
		super.superField++
	}
}
let B = class {
	static {}
	static {
		this.thisField++
		super.superField = super.superField + 1
		super.superField++
	}
}