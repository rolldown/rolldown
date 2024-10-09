# Diff
## /out/a/entry.js
### esbuild
```js
// common.js
var foo = 123;

// a/entry.js
console.log(foo);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = 123;
-console.log(foo);

```
## /out/b/entry.js
### esbuild
```js
// common.js
var foo = 123;

// b/entry.js
console.log(foo);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = 123;
-console.log(foo);

```