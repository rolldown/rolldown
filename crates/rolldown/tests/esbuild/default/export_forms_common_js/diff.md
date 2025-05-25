# Reason
1. redundant `__toCommonJS`
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
const abc = void 0;

//#endregion
//#region b.js
var b_exports = {};
__export(b_exports, { xyz: () => xyz });
const xyz = null;

//#endregion
//#region commonjs.js
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
function Fn() {}
var commonjs_default, v, l, c, Class;
var init_commonjs = __esm({ "commonjs.js"() {
	commonjs_default = 123;
	v = 234;
	l = 234;
	c = 234;
	Class = class {};
} });

//#endregion
//#region c.js
var c_exports = {};
__export(c_exports, { default: () => c_default });
var c_default;
var init_c = __esm({ "c.js"() {
	c_default = class {};
} });

//#endregion
//#region d.js
var d_exports = {};
__export(d_exports, { default: () => Foo });
var Foo;
var init_d = __esm({ "d.js"() {
	Foo = class {};
	Foo.prop = 123;
} });

//#endregion
//#region e.js
var e_exports = {};
__export(e_exports, { default: () => e_default });
function e_default() {}
var init_e = __esm({ "e.js"() {} });

//#endregion
//#region f.js
var f_exports = {};
__export(f_exports, { default: () => foo$1 });
function foo$1() {}
var init_f = __esm({ "f.js"() {
	foo$1.prop = 123;
} });

//#endregion
//#region g.js
var g_exports = {};
__export(g_exports, { default: () => g_default });
async function g_default() {}
var init_g = __esm({ "g.js"() {} });

//#endregion
//#region h.js
var h_exports = {};
__export(h_exports, { default: () => foo });
async function foo() {}
var init_h = __esm({ "h.js"() {
	foo.prop = 123;
} });

//#endregion
//#region entry.js
init_commonjs();
init_c();
init_d();
init_e();
init_f();
init_g();
init_h();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,20 +1,10 @@
-var abc;
-var init_a = __esm({
-    "a.js"() {
-        abc = void 0;
-    }
-});
+var abc = void 0;
 var b_exports = {};
 __export(b_exports, {
     xyz: () => xyz
 });
-var xyz;
-var init_b = __esm({
-    "b.js"() {
-        xyz = null;
-    }
-});
+var xyz = null;
 var commonjs_exports = {};
 __export(commonjs_exports, {
     C: () => Class,
     Class: () => Class,
@@ -29,10 +19,8 @@
 function Fn() {}
 var commonjs_default, v, l, c, Class;
 var init_commonjs = __esm({
     "commonjs.js"() {
-        init_a();
-        init_b();
         commonjs_default = 123;
         v = 234;
         l = 234;
         c = 234;
@@ -69,14 +57,14 @@
     "e.js"() {}
 });
 var f_exports = {};
 __export(f_exports, {
-    default: () => foo
+    default: () => foo$1
 });
-function foo() {}
+function foo$1() {}
 var init_f = __esm({
     "f.js"() {
-        foo.prop = 123;
+        foo$1.prop = 123;
     }
 });
 var g_exports = {};
 __export(g_exports, {
@@ -87,14 +75,14 @@
     "g.js"() {}
 });
 var h_exports = {};
 __export(h_exports, {
-    default: () => foo2
+    default: () => foo
 });
-async function foo2() {}
+async function foo() {}
 var init_h = __esm({
     "h.js"() {
-        foo2.prop = 123;
+        foo.prop = 123;
     }
 });
 init_commonjs();
 init_c();

```