# Diff
## /out.js
### esbuild
```js
var _foo;
class Foo {
  constructor() {
    __privateAdd(this, _foo, 123);
    // This must be set before "bar" is initialized
    __publicField(this, "bar", __privateGet(this, _foo));
  }
}
_foo = new WeakMap();
console.log(new Foo().bar === 123);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region entry.js
class Foo {
	#foo = 123;
	bar = this.#foo;
}
assert.equal(new Foo().bar, 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +1,5 @@
-var _foo;
 class Foo {
-    constructor() {
-        __privateAdd(this, _foo, 123);
-        __publicField(this, "bar", __privateGet(this, _foo));
-    }
+    #foo = 123;
+    bar = this.#foo;
 }
-_foo = new WeakMap();
-console.log(new Foo().bar === 123);
+console.log(new Foo().bar);

```