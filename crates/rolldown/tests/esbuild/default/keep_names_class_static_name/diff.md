# Diff
## /out.js
### esbuild
```js
class A {
  static {
    __name(this, "A");
  }
  static foo;
}
class B {
  static name;
}
class C {
  static name() {
  }
}
class D {
  static get name() {
  }
}
class E {
  static set name(x) {
  }
}
class F {
  static ["name"] = 0;
}
let a = class a3 {
  static {
    __name(this, "a");
  }
  static foo;
};
let b = class b3 {
  static name;
};
let c = class c3 {
  static name() {
  }
};
let d = class d3 {
  static get name() {
  }
};
let e = class e3 {
  static set name(x) {
  }
};
let f = class f3 {
  static ["name"] = 0;
};
let a2 = class {
  static {
    __name(this, "a2");
  }
  static foo;
};
let b2 = class {
  static name;
};
let c2 = class {
  static name() {
  }
};
let d2 = class {
  static get name() {
  }
};
let e2 = class {
  static set name(x) {
  }
};
let f2 = class {
  static ["name"] = 0;
};
```
### rolldown
```js

//#region entry.js
class A {
	static foo;
}
class B {
	static name;
}
class C {
	static name() {}
}
class D {
	static get name() {}
}
class E {
	static set name(x) {}
}
class F {
	static ["name"] = 0;
}
let a = class a$1 {
	static foo;
};
let b = class b$1 {
	static name;
};
let c = class c$1 {
	static name() {}
};
let d = class d$1 {
	static get name() {}
};
let e = class e$1 {
	static set name(x) {}
};
let f = class f$1 {
	static ["name"] = 0;
};
let a2 = class {
	static foo;
};
let b2 = class {
	static name;
};
let c2 = class {
	static name() {}
};
let d2 = class {
	static get name() {}
};
let e2 = class {
	static set name(x) {}
};
let f2 = class {
	static ["name"] = 0;
};

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,5 @@
 class A {
-    static {
-        __name(this, "A");
-    }
     static foo;
 }
 class B {
     static name;
@@ -18,46 +15,40 @@
 }
 class F {
     static ["name"] = 0;
 }
-let a = class a3 {
-    static {
-        __name(this, "a");
-    }
+var a = class a$1 {
     static foo;
 };
-let b = class b3 {
+var b = class b$1 {
     static name;
 };
-let c = class c3 {
+var c = class c$1 {
     static name() {}
 };
-let d = class d3 {
+var d = class d$1 {
     static get name() {}
 };
-let e = class e3 {
+var e = class e$1 {
     static set name(x) {}
 };
-let f = class f3 {
+var f = class f$1 {
     static ["name"] = 0;
 };
-let a2 = class {
-    static {
-        __name(this, "a2");
-    }
+var a2 = class {
     static foo;
 };
-let b2 = class {
+var b2 = class {
     static name;
 };
-let c2 = class {
+var c2 = class {
     static name() {}
 };
-let d2 = class {
+var d2 = class {
     static get name() {}
 };
-let e2 = class {
+var e2 = class {
     static set name(x) {}
 };
-let f2 = class {
+var f2 = class {
     static ["name"] = 0;
 };

```