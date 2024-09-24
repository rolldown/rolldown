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
import { default as assert } from "node:assert";


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
+++ rolldown	entry_js.mjs
@@ -1,5 +1,7 @@
+var foo$1 = 123;
 var bar_exports = {};
-__export(bar_exports, { foo: () => foo });
-var foo = 123;
-var foo2 = 234;
-console.log(bar_exports, foo, foo2);
\ No newline at end of file
+__export(bar_exports, { foo: () => foo$1 });
+var foo = 234;
+console.log(bar_exports);
+console.log(foo$1);
+console.log(foo);
\ No newline at end of file

```
