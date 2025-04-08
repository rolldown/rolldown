# Reason
1. could be done in minifier
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
var A = class {
	static foo;
};
var B = class {
	static name;
};
var C = class {
	static name() {}
};
var D = class {
	static get name() {}
};
var E = class {
	static set name(x) {}
};
var F = class {
	static ["name"] = 0;
};
let a = class a {
	static foo;
};
let b = class b {
	static name;
};
let c = class c {
	static name() {}
};
let d = class d {
	static get name() {}
};
let e = class e {
	static set name(x) {}
};
let f = class f {
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
@@ -1,63 +1,54 @@
-class A {
-    static {
-        __name(this, "A");
-    }
+var A = class {
     static foo;
-}
-class B {
+};
+var B = class {
     static name;
-}
-class C {
+};
+var C = class {
     static name() {}
-}
-class D {
+};
+var D = class {
     static get name() {}
-}
-class E {
+};
+var E = class {
     static set name(x) {}
-}
-class F {
+};
+var F = class {
     static ["name"] = 0;
-}
-let a = class a3 {
-    static {
-        __name(this, "a");
-    }
+};
+var a = class a {
     static foo;
 };
-let b = class b3 {
+var b = class b {
     static name;
 };
-let c = class c3 {
+var c = class c {
     static name() {}
 };
-let d = class d3 {
+var d = class d {
     static get name() {}
 };
-let e = class e3 {
+var e = class e {
     static set name(x) {}
 };
-let f = class f3 {
+var f = class f {
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