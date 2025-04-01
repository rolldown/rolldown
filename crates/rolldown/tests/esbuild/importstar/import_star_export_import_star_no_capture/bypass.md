# Reason
1. rolldown implemented advanced barrel exports opt
# Diff
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
console.log(foo_exports.foo, foo_exports.foo, foo2);
```
### rolldown
```js
import assert from "node:assert";

//#region foo.js
const foo$1 = 123;
//#endregion

//#region entry.js
let foo = 234;
assert.equal(foo$1, 123);
assert.equal(foo, 234);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,3 @@
-var foo_exports = {};
-__export(foo_exports, {
-    foo: () => foo
-});
-var foo = 123;
-var foo2 = 234;
-console.log(foo_exports.foo, foo_exports.foo, foo2);
+var foo$1 = 123;
+var foo = 234;
+console.log(foo$1, foo);

```