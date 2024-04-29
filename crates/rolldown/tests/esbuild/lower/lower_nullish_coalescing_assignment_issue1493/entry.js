export class A {
	#a;
	f() {
		this.#a ??= 1;
	}
}