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
import { default as assert } from "node:assert";

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
+++ rolldown	entry_js.mjs
@@ -1,5 +1,4 @@
-var foo_exports = {};
-__export(foo_exports, { foo: () => foo });
-var foo = 123;
-var foo2 = 234;
-console.log(foo_exports.foo, foo_exports.foo, foo2);
\ No newline at end of file
+const foo$1 = 123;
+let foo = 234;
+console.log(foo$1);
+console.log(foo);
\ No newline at end of file

```
