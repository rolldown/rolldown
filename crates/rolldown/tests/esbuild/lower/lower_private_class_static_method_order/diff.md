# Diff
## /out.js
### esbuild
```js
var _a, _Foo_static, foo_fn, _b, _FooThis_static, foo_fn2;
const _Foo = class _Foo {
  // This must be set before "bar" is initialized
};
_Foo_static = new WeakSet();
foo_fn = function() {
  return 123;
};
__privateAdd(_Foo, _Foo_static);
__publicField(_Foo, "bar", __privateMethod(_a = _Foo, _Foo_static, foo_fn).call(_a));
let Foo = _Foo;
console.log(Foo.bar === 123);
const _FooThis = class _FooThis {
  // This must be set before "bar" is initialized
};
_FooThis_static = new WeakSet();
foo_fn2 = function() {
  return 123;
};
__privateAdd(_FooThis, _FooThis_static);
__publicField(_FooThis, "bar", __privateMethod(_b = _FooThis, _FooThis_static, foo_fn2).call(_b));
let FooThis = _FooThis;
console.log(FooThis.bar === 123);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region entry.js
class Foo {
	static bar = Foo.#foo();
	static #foo() {
		return 123;
	}
}
assert(Foo.bar === 123);
class FooThis {
	static bar = this.#foo();
	static #foo() {
		return 123;
	}
}
assert(FooThis.bar === 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,19 +1,14 @@
-var _a, _Foo_static, foo_fn, _b, _FooThis_static, foo_fn2;
-const _Foo = class _Foo {};
-_Foo_static = new WeakSet();
-foo_fn = function () {
-    return 123;
-};
-__privateAdd(_Foo, _Foo_static);
-__publicField(_Foo, "bar", __privateMethod(_a = _Foo, _Foo_static, foo_fn).call(_a));
-let Foo = _Foo;
-console.log(Foo.bar === 123);
-const _FooThis = class _FooThis {};
-_FooThis_static = new WeakSet();
-foo_fn2 = function () {
-    return 123;
-};
-__privateAdd(_FooThis, _FooThis_static);
-__publicField(_FooThis, "bar", __privateMethod(_b = _FooThis, _FooThis_static, foo_fn2).call(_b));
-let FooThis = _FooThis;
-console.log(FooThis.bar === 123);
+class Foo {
+    static bar = Foo.#foo();
+    static #foo() {
+        return 123;
+    }
+}
+assert(Foo.bar === 123);
+class FooThis {
+    static bar = this.#foo();
+    static #foo() {
+        return 123;
+    }
+}
+assert(FooThis.bar === 123);

```