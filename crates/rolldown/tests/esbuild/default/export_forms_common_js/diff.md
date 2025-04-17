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

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __esm = (fn, res) => function() {
	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region a.js
var abc;
var init_a = __esm({ "a.js"() {
	abc = void 0;
} });

//#region b.js
var b_exports = {};
__export(b_exports, { xyz: () => xyz });
var xyz;
var init_b = __esm({ "b.js"() {
	xyz = null;
} });

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

//#region c.js
var c_exports = {};
__export(c_exports, { default: () => c_default });
var c_default;
var init_c = __esm({ "c.js"() {
	c_default = class {};
} });

//#region d.js
var d_exports = {};
__export(d_exports, { default: () => Foo });
var Foo;
var init_d = __esm({ "d.js"() {
	Foo = class {};
	Foo.prop = 123;
} });

//#region e.js
var e_exports = {};
__export(e_exports, { default: () => e_default });
function e_default() {}
var init_e = __esm({ "e.js"() {} });

//#region f.js
var f_exports = {};
__export(f_exports, { default: () => foo$1 });
function foo$1() {}
var init_f = __esm({ "f.js"() {
	foo$1.prop = 123;
} });

//#region g.js
var g_exports = {};
__export(g_exports, { default: () => g_default });
async function g_default() {}
var init_g = __esm({ "g.js"() {} });

//#region h.js
var h_exports = {};
__export(h_exports, { default: () => foo });
async function foo() {}
var init_h = __esm({ "h.js"() {
	foo.prop = 123;
} });

//#region entry.js
init_commonjs();
init_c();
init_d();
init_e();
init_f();
init_g();
init_h();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,15 @@
+var __defProp = Object.defineProperty;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __esm = (fn, res) => function () {
+    return (fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res);
+};
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
 var abc;
 var init_a = __esm({
     "a.js"() {
         abc = void 0;
@@ -69,14 +80,14 @@
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
@@ -87,14 +98,14 @@
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