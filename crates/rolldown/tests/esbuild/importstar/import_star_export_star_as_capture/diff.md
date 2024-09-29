## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  foo: () => foo
});
var foo = 123;

// entry.js
var foo2 = 234;
console.log(foo_exports, foo_exports.foo, foo2);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region foo.js
var foo_exports = {};
__export(foo_exports, { foo: () => foo$1 });
const foo$1 = 123;

//#endregion
//#region entry.js
let foo = 234;
assert.deepEqual(foo_exports, { foo: 123 });
assert.equal(foo$1, 123);
assert.equal(foo, 234);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,7 +1,7 @@
 var foo_exports = {};
 __export(foo_exports, {
-    foo: () => foo
+    foo: () => foo$1
 });
-var foo = 123;
-var foo2 = 234;
-console.log(foo_exports, foo_exports.foo, foo2);
+var foo$1 = 123;
+var foo = 234;
+console.log(foo_exports, foo$1, foo);

```
