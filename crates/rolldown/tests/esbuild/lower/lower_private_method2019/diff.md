# Diff
## /out.js
### esbuild
```js
// entry.js
var _field, _Foo_instances, method_fn;
var Foo = class {
  constructor() {
    __privateAdd(this, _Foo_instances);
    __privateAdd(this, _field);
  }
  baseline() {
    var _a, _b, _c, _d, _e;
    a().foo;
    b().foo(x);
    (_a = c()) == null ? void 0 : _a.foo(x);
    (_c = (_b = d()).foo) == null ? void 0 : _c.call(_b, x);
    (_e = (_d = e()) == null ? void 0 : _d.foo) == null ? void 0 : _e.call(_d, x);
  }
  privateField() {
    var _a, _b, _c, _d, _e, _f, _g, _h;
    __privateGet(a(), _field);
    __privateGet(_a = b(), _field).call(_a, x);
    (_b = c()) == null ? void 0 : __privateGet(_b, _field).call(_b, x);
    (_d = __privateGet(_c = d(), _field)) == null ? void 0 : _d.call(_c, x);
    (_f = (_e = e()) == null ? void 0 : __privateGet(_e, _field)) == null ? void 0 : _f.call(_e, x);
    (_g = f()) == null ? void 0 : __privateGet(_h = _g.foo, _field).call(_h, x).bar();
  }
  privateMethod() {
    var _a, _b, _c, _d, _e, _f, _g, _h;
    __privateMethod(a(), _Foo_instances, method_fn);
    __privateMethod(_a = b(), _Foo_instances, method_fn).call(_a, x);
    (_b = c()) == null ? void 0 : __privateMethod(_b, _Foo_instances, method_fn).call(_b, x);
    (_d = __privateMethod(_c = d(), _Foo_instances, method_fn)) == null ? void 0 : _d.call(_c, x);
    (_f = (_e = e()) == null ? void 0 : __privateMethod(_e, _Foo_instances, method_fn)) == null ? void 0 : _f.call(_e, x);
    (_g = f()) == null ? void 0 : __privateMethod(_h = _g.foo, _Foo_instances, method_fn).call(_h, x).bar();
  }
};
_field = new WeakMap();
_Foo_instances = new WeakSet();
method_fn = function() {
};
export {
  Foo
};
```
### rolldown
```js

//#region entry.js
class Foo {
	#field;
	#method() {}
	baseline() {
		a().foo;
		b().foo(x);
		c()?.foo(x);
		d().foo?.(x);
		e()?.foo?.(x);
	}
	privateField() {
		a().#field;
		b().#field(x);
		c()?.#field(x);
		d().#field?.(x);
		e()?.#field?.(x);
		f()?.foo.#field(x).bar();
	}
	privateMethod() {
		a().#method;
		b().#method(x);
		c()?.#method(x);
		d().#method?.(x);
		e()?.#method?.(x);
		f()?.foo.#method(x).bar();
	}
}

//#endregion
export { Foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,37 +1,28 @@
-var _field, _Foo_instances, method_fn;
-var Foo = class {
-    constructor() {
-        __privateAdd(this, _Foo_instances);
-        __privateAdd(this, _field);
-    }
+class Foo {
+    #field;
+    #method() {}
     baseline() {
-        var _a, _b, _c, _d, _e;
         a().foo;
         b().foo(x);
-        (_a = c()) == null ? void 0 : _a.foo(x);
-        (_c = (_b = d()).foo) == null ? void 0 : _c.call(_b, x);
-        (_e = (_d = e()) == null ? void 0 : _d.foo) == null ? void 0 : _e.call(_d, x);
+        c()?.foo(x);
+        d().foo?.(x);
+        e()?.foo?.(x);
     }
     privateField() {
-        var _a, _b, _c, _d, _e, _f, _g, _h;
-        __privateGet(a(), _field);
-        __privateGet(_a = b(), _field).call(_a, x);
-        (_b = c()) == null ? void 0 : __privateGet(_b, _field).call(_b, x);
-        (_d = __privateGet(_c = d(), _field)) == null ? void 0 : _d.call(_c, x);
-        (_f = (_e = e()) == null ? void 0 : __privateGet(_e, _field)) == null ? void 0 : _f.call(_e, x);
-        (_g = f()) == null ? void 0 : __privateGet(_h = _g.foo, _field).call(_h, x).bar();
+        a().#field;
+        b().#field(x);
+        c()?.#field(x);
+        d().#field?.(x);
+        e()?.#field?.(x);
+        f()?.foo.#field(x).bar();
     }
     privateMethod() {
-        var _a, _b, _c, _d, _e, _f, _g, _h;
-        __privateMethod(a(), _Foo_instances, method_fn);
-        __privateMethod(_a = b(), _Foo_instances, method_fn).call(_a, x);
-        (_b = c()) == null ? void 0 : __privateMethod(_b, _Foo_instances, method_fn).call(_b, x);
-        (_d = __privateMethod(_c = d(), _Foo_instances, method_fn)) == null ? void 0 : _d.call(_c, x);
-        (_f = (_e = e()) == null ? void 0 : __privateMethod(_e, _Foo_instances, method_fn)) == null ? void 0 : _f.call(_e, x);
-        (_g = f()) == null ? void 0 : __privateMethod(_h = _g.foo, _Foo_instances, method_fn).call(_h, x).bar();
+        a().#method;
+        b().#method(x);
+        c()?.#method(x);
+        d().#method?.(x);
+        e()?.#method?.(x);
+        f()?.foo.#method(x).bar();
     }
-};
-_field = new WeakMap();
-_Foo_instances = new WeakSet();
-method_fn = function () {};
+}
 export {Foo};

```