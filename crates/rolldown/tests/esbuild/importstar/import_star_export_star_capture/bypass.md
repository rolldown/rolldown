# Reason
1. different deconflict naming style and order
# Diff
## /out.js
### esbuild
```js
// bar.js
var bar_exports = {};
__export(bar_exports, {
  foo: () => foo
});

// foo.js
var foo = 123;

// entry.js
var foo2 = 234;
console.log(bar_exports, foo, foo2);
```
### rolldown
```js
import assert from "node:assert";



//#region foo.js
const foo$1 = 123;
//#endregion

//#region bar.js
var bar_exports = {};
__export(bar_exports, { foo: () => foo$1 });
//#endregion

//#region entry.js
let foo = 234;
assert.deepEqual(bar_exports, { foo: 123 });
assert.equal(foo$1, 123);
assert.equal(foo, 234);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
+var foo$1 = 123;
 var bar_exports = {};
 __export(bar_exports, {
-    foo: () => foo
+    foo: () => foo$1
 });
-var foo = 123;
-var foo2 = 234;
-console.log(bar_exports, foo, foo2);
+var foo = 234;
+console.log(bar_exports, foo$1, foo);

```