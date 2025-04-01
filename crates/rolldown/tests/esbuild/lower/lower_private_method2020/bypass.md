# Reason
1. pure transformation is handled by `oxc-transform`
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
    a().foo;
    b().foo(x);
    c()?.foo(x);
    d().foo?.(x);
    e()?.foo?.(x);
  }
  privateField() {
    var _a, _b, _c, _d, _e, _f, _g;
    __privateGet(a(), _field);
    __privateGet(_a = b(), _field).call(_a, x);
    (_b = c()) == null ? void 0 : __privateGet(_b, _field).call(_b, x);
    (_d = __privateGet(_c = d(), _field)) == null ? void 0 : _d.call(_c, x);
    ((_e = e()) == null ? void 0 : __privateGet(_e, _field))?.(x);
    (_f = f()) == null ? void 0 : __privateGet(_g = _f.foo, _field).call(_g, x).bar();
  }
  privateMethod() {
    var _a, _b, _c, _d, _e, _f, _g;
    __privateMethod(a(), _Foo_instances, method_fn);
    __privateMethod(_a = b(), _Foo_instances, method_fn).call(_a, x);
    (_b = c()) == null ? void 0 : __privateMethod(_b, _Foo_instances, method_fn).call(_b, x);
    (_d = __privateMethod(_c = d(), _Foo_instances, method_fn)) == null ? void 0 : _d.call(_c, x);
    ((_e = e()) == null ? void 0 : __privateMethod(_e, _Foo_instances, method_fn))?.(x);
    (_f = f()) == null ? void 0 : __privateMethod(_g = _f.foo, _Foo_instances, method_fn).call(_g, x).bar();
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
var Foo = class {
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
};
//#endregion

export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,36 +1,28 @@
-var _field, _Foo_instances, method_fn;
 var Foo = class {
-    constructor() {
-        __privateAdd(this, _Foo_instances);
-        __privateAdd(this, _field);
-    }
+    #field;
+    #method() {}
     baseline() {
         a().foo;
         b().foo(x);
         c()?.foo(x);
         d().foo?.(x);
         e()?.foo?.(x);
     }
     privateField() {
-        var _a, _b, _c, _d, _e, _f, _g;
-        __privateGet(a(), _field);
-        __privateGet(_a = b(), _field).call(_a, x);
-        (_b = c()) == null ? void 0 : __privateGet(_b, _field).call(_b, x);
-        (_d = __privateGet(_c = d(), _field)) == null ? void 0 : _d.call(_c, x);
-        ((_e = e()) == null ? void 0 : __privateGet(_e, _field))?.(x);
-        (_f = f()) == null ? void 0 : __privateGet(_g = _f.foo, _field).call(_g, x).bar();
+        a().#field;
+        b().#field(x);
+        c()?.#field(x);
+        d().#field?.(x);
+        e()?.#field?.(x);
+        f()?.foo.#field(x).bar();
     }
     privateMethod() {
-        var _a, _b, _c, _d, _e, _f, _g;
-        __privateMethod(a(), _Foo_instances, method_fn);
-        __privateMethod(_a = b(), _Foo_instances, method_fn).call(_a, x);
-        (_b = c()) == null ? void 0 : __privateMethod(_b, _Foo_instances, method_fn).call(_b, x);
-        (_d = __privateMethod(_c = d(), _Foo_instances, method_fn)) == null ? void 0 : _d.call(_c, x);
-        ((_e = e()) == null ? void 0 : __privateMethod(_e, _Foo_instances, method_fn))?.(x);
-        (_f = f()) == null ? void 0 : __privateMethod(_g = _f.foo, _Foo_instances, method_fn).call(_g, x).bar();
+        a().#method;
+        b().#method(x);
+        c()?.#method(x);
+        d().#method?.(x);
+        e()?.#method?.(x);
+        f()?.foo.#method(x).bar();
     }
 };
-_field = new WeakMap();
-_Foo_instances = new WeakSet();
-method_fn = function () {};
 export {Foo};

```