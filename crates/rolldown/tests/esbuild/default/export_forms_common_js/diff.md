# Diff
## /out.js
### esbuild
```js
// a.js
var abc;
var init_a = __esm({
  "a.js"() {
    abc = void 0;
  }
});

// b.js
var b_exports = {};
__export(b_exports, {
  xyz: () => xyz
});
var xyz;
var init_b = __esm({
  "b.js"() {
    xyz = null;
  }
});

// commonjs.js
var commonjs_exports = {};
__export(commonjs_exports, {
  C: () => Class,
  Class: () => Class,
  Fn: () => Fn,
  abc: () => abc,
  b: () => b_exports,
  c: () => c,
  default: () => commonjs_default,
  l: () => l,
  v: () => v
});
function Fn() {
}
var commonjs_default, v, l, c, Class;
var init_commonjs = __esm({
  "commonjs.js"() {
    init_a();
    init_b();
    commonjs_default = 123;
    v = 234;
    l = 234;
    c = 234;
    Class = class {
    };
  }
});

// c.js
var c_exports = {};
__export(c_exports, {
  default: () => c_default
});
var c_default;
var init_c = __esm({
  "c.js"() {
    c_default = class {
    };
  }
});

// d.js
var d_exports = {};
__export(d_exports, {
  default: () => Foo
});
var Foo;
var init_d = __esm({
  "d.js"() {
    Foo = class {
    };
    Foo.prop = 123;
  }
});

// e.js
var e_exports = {};
__export(e_exports, {
  default: () => e_default
});
function e_default() {
}
var init_e = __esm({
  "e.js"() {
  }
});

// f.js
var f_exports = {};
__export(f_exports, {
  default: () => foo
});
function foo() {
}
var init_f = __esm({
  "f.js"() {
    foo.prop = 123;
  }
});

// g.js
var g_exports = {};
__export(g_exports, {
  default: () => g_default
});
async function g_default() {
}
var init_g = __esm({
  "g.js"() {
  }
});

// h.js
var h_exports = {};
__export(h_exports, {
  default: () => foo2
});
async function foo2() {
}
var init_h = __esm({
  "h.js"() {
    foo2.prop = 123;
  }
});

// entry.js
init_commonjs();
init_c();
init_d();
init_e();
init_f();
init_g();
init_h();
```
### rolldown
```js


//#region a.js
var abc;
var init_a = __esm({ "a.js"() {
	abc = undefined;
} });

//#endregion
//#region b.js
var b_exports, xyz;
var init_b = __esm({ "b.js"() {
	b_exports = {};
	__export(b_exports, { xyz: () => xyz });
	xyz = null;
} });

//#endregion
//#region commonjs.js
function Fn() {}
var commonjs_exports, commonjs_default, v, l, c, Class;
var init_commonjs = __esm({ "commonjs.js"() {
	commonjs_exports = {};
	__export(commonjs_exports, {
		C: () => Class,
		Class: () => Class,
		Fn: () => Fn,
		abc: () => abc,
		b: () => b_exports,
		c: () => c,
		default: () => commonjs_default,
		l: () => l,
		v: () => v
	});
	init_a();
	init_b();
	commonjs_default = 123;
	v = 234;
	l = 234;
	c = 234;
	Class = class {};
} });

//#endregion
//#region c.js
var c_exports, c_default;
var init_c = __esm({ "c.js"() {
	c_exports = {};
	__export(c_exports, { default: () => c_default });
	c_default = class {};
} });

//#endregion
//#region d.js
var d_exports, Foo;
var init_d = __esm({ "d.js"() {
	d_exports = {};
	__export(d_exports, { default: () => Foo });
	Foo = class {};
	Foo.prop = 123;
} });

//#endregion
//#region e.js
function e_default() {}
var e_exports;
var init_e = __esm({ "e.js"() {
	e_exports = {};
	__export(e_exports, { default: () => e_default });
} });

//#endregion
//#region f.js
function foo$1() {}
var f_exports;
var init_f = __esm({ "f.js"() {
	f_exports = {};
	__export(f_exports, { default: () => foo$1 });
	foo$1.prop = 123;
} });

//#endregion
//#region g.js
async function g_default() {}
var g_exports;
var init_g = __esm({ "g.js"() {
	g_exports = {};
	__export(g_exports, { default: () => g_default });
} });

//#endregion
//#region h.js
async function foo() {}
var h_exports;
var init_h = __esm({ "h.js"() {
	h_exports = {};
	__export(h_exports, { default: () => foo });
	foo.prop = 123;
} });

//#endregion
//#region entry.js
init_commonjs(), __toCommonJS(commonjs_exports);
init_c(), __toCommonJS(c_exports);
init_d(), __toCommonJS(d_exports);
init_e(), __toCommonJS(e_exports);
init_f(), __toCommonJS(f_exports);
init_g(), __toCommonJS(g_exports);
init_h(), __toCommonJS(h_exports);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,36 +1,36 @@
 var abc;
 var init_a = __esm({
     "a.js"() {
-        abc = void 0;
+        abc = undefined;
     }
 });
-var b_exports = {};
-__export(b_exports, {
-    xyz: () => xyz
-});
-var xyz;
+var b_exports, xyz;
 var init_b = __esm({
     "b.js"() {
+        b_exports = {};
+        __export(b_exports, {
+            xyz: () => xyz
+        });
         xyz = null;
     }
 });
-var commonjs_exports = {};
-__export(commonjs_exports, {
-    C: () => Class,
-    Class: () => Class,
-    Fn: () => Fn,
-    abc: () => abc,
-    b: () => b_exports,
-    c: () => c,
-    default: () => commonjs_default,
-    l: () => l,
-    v: () => v
-});
 function Fn() {}
-var commonjs_default, v, l, c, Class;
+var commonjs_exports, commonjs_default, v, l, c, Class;
 var init_commonjs = __esm({
     "commonjs.js"() {
+        commonjs_exports = {};
+        __export(commonjs_exports, {
+            C: () => Class,
+            Class: () => Class,
+            Fn: () => Fn,
+            abc: () => abc,
+            b: () => b_exports,
+            c: () => c,
+            default: () => commonjs_default,
+            l: () => l,
+            v: () => v
+        });
         init_a();
         init_b();
         commonjs_default = 123;
         v = 234;
@@ -38,68 +38,74 @@
         c = 234;
         Class = class {};
     }
 });
-var c_exports = {};
-__export(c_exports, {
-    default: () => c_default
-});
-var c_default;
+var c_exports, c_default;
 var init_c = __esm({
     "c.js"() {
+        c_exports = {};
+        __export(c_exports, {
+            default: () => c_default
+        });
         c_default = class {};
     }
 });
-var d_exports = {};
-__export(d_exports, {
-    default: () => Foo
-});
-var Foo;
+var d_exports, Foo;
 var init_d = __esm({
     "d.js"() {
+        d_exports = {};
+        __export(d_exports, {
+            default: () => Foo
+        });
         Foo = class {};
         Foo.prop = 123;
     }
 });
-var e_exports = {};
-__export(e_exports, {
-    default: () => e_default
-});
 function e_default() {}
+var e_exports;
 var init_e = __esm({
-    "e.js"() {}
+    "e.js"() {
+        e_exports = {};
+        __export(e_exports, {
+            default: () => e_default
+        });
+    }
 });
-var f_exports = {};
-__export(f_exports, {
-    default: () => foo
-});
-function foo() {}
+function foo$1() {}
+var f_exports;
 var init_f = __esm({
     "f.js"() {
-        foo.prop = 123;
+        f_exports = {};
+        __export(f_exports, {
+            default: () => foo$1
+        });
+        foo$1.prop = 123;
     }
 });
-var g_exports = {};
-__export(g_exports, {
-    default: () => g_default
-});
 async function g_default() {}
+var g_exports;
 var init_g = __esm({
-    "g.js"() {}
+    "g.js"() {
+        g_exports = {};
+        __export(g_exports, {
+            default: () => g_default
+        });
+    }
 });
-var h_exports = {};
-__export(h_exports, {
-    default: () => foo2
-});
-async function foo2() {}
+async function foo() {}
+var h_exports;
 var init_h = __esm({
     "h.js"() {
-        foo2.prop = 123;
+        h_exports = {};
+        __export(h_exports, {
+            default: () => foo
+        });
+        foo.prop = 123;
     }
 });
-init_commonjs();
-init_c();
-init_d();
-init_e();
-init_f();
-init_g();
-init_h();
+(init_commonjs(), __toCommonJS(commonjs_exports));
+(init_c(), __toCommonJS(c_exports));
+(init_d(), __toCommonJS(d_exports));
+(init_e(), __toCommonJS(e_exports));
+(init_f(), __toCommonJS(f_exports));
+(init_g(), __toCommonJS(g_exports));
+(init_h(), __toCommonJS(h_exports));

```