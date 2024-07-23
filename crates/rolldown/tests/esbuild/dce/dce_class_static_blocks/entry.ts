class A_REMOVE {
	static {}
}
class B_REMOVE {
	static { 123 }
}
class C_REMOVE {
	static { /* @__PURE__*/ foo() }
}
class D_REMOVE {
	static { try {} catch {} }
}
class E_REMOVE {
	static { try { /* @__PURE__*/ foo() } catch {} }
}
class F_REMOVE {
	static { try { 123 } catch { 123 } finally { 123 } }
}

class A_keep {
	static { foo }
}
class B_keep {
	static { this.foo }
}
class C_keep {
	static { try { foo } catch {} }
}
class D_keep {
	static { try {} finally { foo } }
}