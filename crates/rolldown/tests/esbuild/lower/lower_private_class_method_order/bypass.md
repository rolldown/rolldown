# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
var _Foo_instances, foo_fn;
class Foo {
  constructor() {
    __privateAdd(this, _Foo_instances);
    __publicField(this, "bar", __privateMethod(this, _Foo_instances, foo_fn).call(this));
  }
  // This must be set before "bar" is initialized
}
_Foo_instances = new WeakSet();
foo_fn = function() {
  return 123;
};
console.log(new Foo().bar === 123);
```
### rolldown
```js
import assert from "node:assert";

//#region entry.js
var Foo = class {
	bar = this.#foo();
	#foo() {
		return 123;
	}
};
assert.equal(new Foo().bar, 123);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,7 @@
-var _Foo_instances, foo_fn;
-class Foo {
-    constructor() {
-        __privateAdd(this, _Foo_instances);
-        __publicField(this, "bar", __privateMethod(this, _Foo_instances, foo_fn).call(this));
+var Foo = class {
+    bar = this.#foo();
+    #foo() {
+        return 123;
     }
-}
-_Foo_instances = new WeakSet();
-foo_fn = function () {
-    return 123;
 };
-console.log(new Foo().bar === 123);
+console.log(new Foo().bar);

```