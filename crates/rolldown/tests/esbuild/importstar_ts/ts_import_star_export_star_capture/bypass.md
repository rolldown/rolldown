# Reason
1. different deconflict naming style and order
# Diff
## /out.js
### esbuild
```js
// bar.ts
var bar_exports = {};
__export(bar_exports, {
  foo: () => foo
});

// foo.ts
var foo = 123;

// entry.ts
var foo2 = 234;
console.log(bar_exports, foo, foo2);
```
### rolldown
```js

//#region foo.ts
const foo$1 = 123;

//#endregion
//#region bar.ts
var bar_exports = {};
__export(bar_exports, { foo: () => foo$1 });

//#endregion
//#region entry.ts
let foo = 234;
console.log(bar_exports, foo$1, foo);

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