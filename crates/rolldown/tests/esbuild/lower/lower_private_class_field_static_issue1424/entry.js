import assert from 'node:assert'
class T {
	#a() { return 'a'; }
	#b() { return 'b'; }
	static c;
	d() { assert.equal(this.#a(), 'a'); }
}
new T().d();
