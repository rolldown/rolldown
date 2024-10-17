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
var abc;
var init_a = __esm({ "a.js"() {
	abc = undefined;
} });

//#endregion
//#region b.js
var b_exports = {};
__export(b_exports, { xyz: () => xyz });
var xyz;
var init_b = __esm({ "b.js"() {
	xyz = null;
} });

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
@@ -1,8 +1,8 @@
 var abc;
 var init_a = __esm({
     "a.js"() {
-        abc = void 0;
+        abc = undefined;
     }
 });
 var b_exports = {};
 __export(b_exports, {
@@ -69,14 +69,14 @@
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
@@ -87,19 +87,19 @@
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