# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
var _Foo_static, foo_get, _FooThis_static, foo_get2;
const _Foo = class _Foo {
  // This must be set before "bar" is initialized
};
_Foo_static = new WeakSet();
foo_get = function() {
  return 123;
};
__privateAdd(_Foo, _Foo_static);
__publicField(_Foo, "bar", __privateGet(_Foo, _Foo_static, foo_get));
let Foo = _Foo;
console.log(Foo.bar === 123);
const _FooThis = class _FooThis {
  // This must be set before "bar" is initialized
};
_FooThis_static = new WeakSet();
foo_get2 = function() {
  return 123;
};
__privateAdd(_FooThis, _FooThis_static);
__publicField(_FooThis, "bar", __privateGet(_FooThis, _FooThis_static, foo_get2));
let FooThis = _FooThis;
console.log(FooThis.bar === 123);
```
### rolldown
```js
import assert from "node:assert";

//#region entry.js
var Foo = class Foo {
	static bar = Foo.#foo;
	static get #foo() {
		return 123;
	}
};
assert.equal(Foo.bar, 123);
var FooThis = class {
	static bar = this.#foo;
	static get #foo() {
		return 123;
	}
};
assert.equal(FooThis.bar, 123);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,19 +1,13 @@
-var _Foo_static, foo_get, _FooThis_static, foo_get2;
-const _Foo = class _Foo {};
-_Foo_static = new WeakSet();
-foo_get = function () {
-    return 123;
+var Foo = class Foo {
+    static bar = Foo.#foo;
+    static get #foo() {
+        return 123;
+    }
 };
-__privateAdd(_Foo, _Foo_static);
-__publicField(_Foo, "bar", __privateGet(_Foo, _Foo_static, foo_get));
-let Foo = _Foo;
-console.log(Foo.bar === 123);
-const _FooThis = class _FooThis {};
-_FooThis_static = new WeakSet();
-foo_get2 = function () {
-    return 123;
+var FooThis = class {
+    static bar = this.#foo;
+    static get #foo() {
+        return 123;
+    }
 };
-__privateAdd(_FooThis, _FooThis_static);
-__publicField(_FooThis, "bar", __privateGet(_FooThis, _FooThis_static, foo_get2));
-let FooThis = _FooThis;
-console.log(FooThis.bar === 123);
+console.log(Foo.bar, FooThis.bar);

```