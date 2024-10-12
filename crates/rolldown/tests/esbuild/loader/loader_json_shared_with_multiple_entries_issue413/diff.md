# Diff
## /out/a.js
### esbuild
```js
// data.json
var data_default = { test: 123 };

// a.js
console.log("a:", data_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var data_default = {
-    test: 123
-};
-console.log("a:", data_default);

```
## /out/b.js
### esbuild
```js
// data.json
var data_default = { test: 123 };

// b.js
console.log("b:", data_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var data_default = {
-    test: 123
-};
-console.log("b:", data_default);

```