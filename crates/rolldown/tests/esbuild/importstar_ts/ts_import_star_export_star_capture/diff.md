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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var bar_exports = {};
-__export(bar_exports, {
-    foo: () => foo
-});
-var foo = 123;
-var foo2 = 234;
-console.log(bar_exports, foo, foo2);

```