# Diff
## /out.js
### esbuild
```js
var _field, _Foo_instances, method_fn, _a, _staticField, _Foo_static, staticMethod_fn;
export let Foo = (_a = class {
  constructor() {
    __privateAdd(this, _Foo_instances);
    __privateAdd(this, _field);
  }
  foo() {
    var _a2;
    __privateSet(this, _field, __privateMethod(this, _Foo_instances, method_fn).call(this));
    __privateSet(Foo, _staticField, __privateMethod(_a2 = Foo, _Foo_static, staticMethod_fn).call(_a2));
  }
}, _field = new WeakMap(), _Foo_instances = new WeakSet(), method_fn = function() {
}, _staticField = new WeakMap(), _Foo_static = new WeakSet(), staticMethod_fn = function() {
}, __privateAdd(_a, _Foo_static), __privateAdd(_a, _staticField), _a);
```
### rolldown
```js

//#region entry.js
let Foo = class {
	#field;
	#method() {}
	static #staticField;
	static #staticMethod() {}
	foo() {
		this.#field = this.#method();
		Foo.#staticField = Foo.#staticMethod();
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
@@ -1,12 +1,11 @@
-var _field, _Foo_instances, method_fn, _a, _staticField, _Foo_static, staticMethod_fn;
-export let Foo = (_a = class {
-    constructor() {
-        __privateAdd(this, _Foo_instances);
-        __privateAdd(this, _field);
-    }
+var Foo = class {
+    #field;
+    #method() {}
+    static #staticField;
+    static #staticMethod() {}
     foo() {
-        var _a2;
-        __privateSet(this, _field, __privateMethod(this, _Foo_instances, method_fn).call(this));
-        __privateSet(Foo, _staticField, __privateMethod(_a2 = Foo, _Foo_static, staticMethod_fn).call(_a2));
+        this.#field = this.#method();
+        Foo.#staticField = Foo.#staticMethod();
     }
-}, _field = new WeakMap(), _Foo_instances = new WeakSet(), method_fn = function () {}, _staticField = new WeakMap(), _Foo_static = new WeakSet(), staticMethod_fn = function () {}, __privateAdd(_a, _Foo_static), __privateAdd(_a, _staticField), _a);
+};
+export {Foo};

```