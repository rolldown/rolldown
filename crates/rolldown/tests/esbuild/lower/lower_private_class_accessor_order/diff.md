# Diff
## /out.js
### esbuild
```js
var _Foo_instances, foo_get;
class Foo {
  constructor() {
    __privateAdd(this, _Foo_instances);
    __publicField(this, "bar", __privateGet(this, _Foo_instances, foo_get));
  }
  // This must be set before "bar" is initialized
}
_Foo_instances = new WeakSet();
foo_get = function() {
  return 123;
};
console.log(new Foo().bar === 123);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region entry.js
class Foo {
	bar = this.#foo;
	get #foo() {
		return 123;
	}
}
assert(new Foo().bar === 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,7 @@
-var _Foo_instances, foo_get;
 class Foo {
-    constructor() {
-        __privateAdd(this, _Foo_instances);
-        __publicField(this, "bar", __privateGet(this, _Foo_instances, foo_get));
+    bar = this.#foo;
+    get #foo() {
+        return 123;
     }
 }
-_Foo_instances = new WeakSet();
-foo_get = function () {
-    return 123;
-};
-console.log(new Foo().bar === 123);
+assert(new Foo().bar === 123);

```