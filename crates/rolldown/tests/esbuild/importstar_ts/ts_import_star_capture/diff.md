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
console.log(foo_exports, foo, foo2);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var foo_exports = {};
-__export(foo_exports, {
-    foo: () => foo
-});
-var foo = 123;
-var foo2 = 234;
-console.log(foo_exports, foo, foo2);

```