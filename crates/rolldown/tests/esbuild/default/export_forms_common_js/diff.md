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
// HIDDEN [rolldown:runtime]
//#region a.js
var abc;
var init_a = __esmMin((() => {
	abc = void 0;
}));

//#endregion
//#region b.js
var b_exports = /* @__PURE__ */ __export({ xyz: () => xyz });
var xyz;
var init_b = __esmMin((() => {
	xyz = null;
}));

//#endregion
//#region commonjs.js
var commonjs_exports = /* @__PURE__ */ __export({
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
var init_commonjs = __esmMin((() => {
	init_a();
	init_b();
	commonjs_default = 123;
	v = 234;
	l = 234;
	c = 234;
	Class = class {};
}));

//#endregion
//#region c.js
var c_exports = /* @__PURE__ */ __export({ default: () => c_default });
var c_default;
var init_c = __esmMin((() => {
	c_default = class {};
}));

//#endregion
//#region d.js
var d_exports = /* @__PURE__ */ __export({ default: () => Foo });
var Foo;
var init_d = __esmMin((() => {
	Foo = class {};
	Foo.prop = 123;
}));

//#endregion
//#region e.js
var e_exports = /* @__PURE__ */ __export({ default: () => e_default });
function e_default() {}
var init_e = __esmMin((() => {}));

//#endregion
//#region f.js
var f_exports = /* @__PURE__ */ __export({ default: () => foo$1 });
function foo$1() {}
var init_f = __esmMin((() => {
	foo$1.prop = 123;
}));

//#endregion
//#region g.js
var g_exports = /* @__PURE__ */ __export({ default: () => g_default });
async function g_default() {}
var init_g = __esmMin((() => {}));

//#endregion
//#region h.js
var h_exports = /* @__PURE__ */ __export({ default: () => foo });
async function foo() {}
var init_h = __esmMin((() => {
	foo.prop = 123;
}));

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
@@ -1,22 +1,16 @@
 var abc;
-var init_a = __esm({
-    "a.js"() {
-        abc = void 0;
-    }
+var init_a = __esmMin(() => {
+    abc = void 0;
 });
-var b_exports = {};
-__export(b_exports, {
+var b_exports = __export({
     xyz: () => xyz
 });
 var xyz;
-var init_b = __esm({
-    "b.js"() {
-        xyz = null;
-    }
+var init_b = __esmMin(() => {
+    xyz = null;
 });
-var commonjs_exports = {};
-__export(commonjs_exports, {
+var commonjs_exports = __export({
     C: () => Class,
     Class: () => Class,
     Fn: () => Fn,
     abc: () => abc,
@@ -27,76 +21,56 @@
     v: () => v
 });
 function Fn() {}
 var commonjs_default, v, l, c, Class;
-var init_commonjs = __esm({
-    "commonjs.js"() {
-        init_a();
-        init_b();
-        commonjs_default = 123;
-        v = 234;
-        l = 234;
-        c = 234;
-        Class = class {};
-    }
+var init_commonjs = __esmMin(() => {
+    init_a();
+    init_b();
+    commonjs_default = 123;
+    v = 234;
+    l = 234;
+    c = 234;
+    Class = class {};
 });
-var c_exports = {};
-__export(c_exports, {
+var c_exports = __export({
     default: () => c_default
 });
 var c_default;
-var init_c = __esm({
-    "c.js"() {
-        c_default = class {};
-    }
+var init_c = __esmMin(() => {
+    c_default = class {};
 });
-var d_exports = {};
-__export(d_exports, {
+var d_exports = __export({
     default: () => Foo
 });
 var Foo;
-var init_d = __esm({
-    "d.js"() {
-        Foo = class {};
-        Foo.prop = 123;
-    }
+var init_d = __esmMin(() => {
+    Foo = class {};
+    Foo.prop = 123;
 });
-var e_exports = {};
-__export(e_exports, {
+var e_exports = __export({
     default: () => e_default
 });
 function e_default() {}
-var init_e = __esm({
-    "e.js"() {}
+var init_e = __esmMin(() => {});
+var f_exports = __export({
+    default: () => foo$1
 });
-var f_exports = {};
-__export(f_exports, {
-    default: () => foo
+function foo$1() {}
+var init_f = __esmMin(() => {
+    foo$1.prop = 123;
 });
-function foo() {}
-var init_f = __esm({
-    "f.js"() {
-        foo.prop = 123;
-    }
-});
-var g_exports = {};
-__export(g_exports, {
+var g_exports = __export({
     default: () => g_default
 });
 async function g_default() {}
-var init_g = __esm({
-    "g.js"() {}
+var init_g = __esmMin(() => {});
+var h_exports = __export({
+    default: () => foo
 });
-var h_exports = {};
-__export(h_exports, {
-    default: () => foo2
+async function foo() {}
+var init_h = __esmMin(() => {
+    foo.prop = 123;
 });
-async function foo2() {}
-var init_h = __esm({
-    "h.js"() {
-        foo2.prop = 123;
-    }
-});
 init_commonjs();
 init_c();
 init_d();
 init_e();

```