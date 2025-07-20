# Reason
1. different deconflict naming style and order 
2. rolldown support advance barrel exports opt
# Diff
## /out.js
### esbuild
```js
// foo.ts
var foo_exports = {};
__export(foo_exports, {
  foo: () => foo
});
var foo = 123;

// entry.ts
var foo2 = 234;
console.log(foo_exports, foo_exports.foo, foo2);
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region foo.ts
var foo_exports = {};
__export(foo_exports, { foo: () => foo$1 });
const foo$1 = 123;

//#endregion
//#region entry.ts
let foo = 234;
console.log(foo_exports, foo$1, foo);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
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