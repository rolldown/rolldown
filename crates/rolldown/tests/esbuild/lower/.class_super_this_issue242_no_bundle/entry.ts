export class A {}

export class B extends A {
	#e: string
	constructor(c: { d: any }) {
		super()
		this.#e = c.d ?? 'test'
	}
	f() {
		return this.#e
	}
}