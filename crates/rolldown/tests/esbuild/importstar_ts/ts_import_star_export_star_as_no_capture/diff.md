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
console.log(foo_exports.foo, foo_exports.foo, foo2);
```
### rolldown
```js

//#region foo.ts
const foo$1 = 123;

//#endregion
//#region entry.ts
let foo = 234;
console.log(foo$1, foo$1, foo);

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
+console.log(foo$1, foo$1, foo);

```