class T {
	#a() { return 'a'; }
	#b() { return 'b'; }
	static c;
	d() { console.log(this.#a()); }
}
new T().d();