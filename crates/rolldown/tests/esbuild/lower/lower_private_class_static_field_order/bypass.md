# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
var _foo, _foo2;
const _Foo = class _Foo {
};
_foo = new WeakMap();
__privateAdd(_Foo, _foo, 123);
// This must be set before "bar" is initialized
__publicField(_Foo, "bar", __privateGet(_Foo, _foo));
let Foo = _Foo;
console.log(Foo.bar === 123);
const _FooThis = class _FooThis {
};
_foo2 = new WeakMap();
__privateAdd(_FooThis, _foo2, 123);
// This must be set before "bar" is initialized
__publicField(_FooThis, "bar", __privateGet(_FooThis, _foo2));
let FooThis = _FooThis;
console.log(FooThis.bar === 123);
```
### rolldown
```js
import assert from "node:assert";

//#region entry.js
var Foo = class Foo {
	static #foo = 123;
	static bar = Foo.#foo;
};
assert.equal(Foo.bar, 123);
var FooThis = class {
	static #foo = 123;
	static bar = this.#foo;
};
assert.equal(FooThis.bar, 123);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,13 +1,9 @@
-var _foo, _foo2;
-const _Foo = class _Foo {};
-_foo = new WeakMap();
-__privateAdd(_Foo, _foo, 123);
-__publicField(_Foo, "bar", __privateGet(_Foo, _foo));
-let Foo = _Foo;
-console.log(Foo.bar === 123);
-const _FooThis = class _FooThis {};
-_foo2 = new WeakMap();
-__privateAdd(_FooThis, _foo2, 123);
-__publicField(_FooThis, "bar", __privateGet(_FooThis, _foo2));
-let FooThis = _FooThis;
-console.log(FooThis.bar === 123);
+var Foo = class Foo {
+    static #foo = 123;
+    static bar = Foo.#foo;
+};
+var FooThis = class {
+    static #foo = 123;
+    static bar = this.#foo;
+};
+console.log(Foo.bar, FooThis.bar);

```