# Diff
## /out/top-level-no-eval.js
### esbuild
```js
const x = 1;
console.log(1, evil("x"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/top-level-no-eval.js
+++ rolldown	
@@ -1,2 +0,0 @@
-const x = 1;
-console.log(1, evil("x"));

```
## /out/top-level-eval.js
### esbuild
```js
const x = 1;
console.log(1, eval("x"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/top-level-eval.js
+++ rolldown	
@@ -1,2 +0,0 @@
-const x = 1;
-console.log(1, eval("x"));

```
## /out/nested-no-eval.js
### esbuild
```js
console.log(1, evil("x"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-no-eval.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(1, evil("x"));

```
## /out/nested-eval.js
### esbuild
```js
(() => {
  const x = 1;
  console.log(1, eval("x"));
})();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-eval.js
+++ rolldown	
@@ -1,4 +0,0 @@
-(() => {
-    const x = 1;
-    console.log(1, eval("x"));
-})();

```
## /out/ts-namespace-no-eval.js
### esbuild
```js
var y;
((y2) => (y2.x = 1, console.log(1, evil("x"))))(y ||= {});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ts-namespace-no-eval.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var y;
-(y2 => (y2.x = 1, console.log(1, evil("x"))))(y ||= {});

```
## /out/ts-namespace-eval.js
### esbuild
```js
var z;
((z) => (z.x = 1, console.log(1, eval("x"))))(z ||= {});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ts-namespace-eval.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var z;
-(z => (z.x = 1, console.log(1, eval("x"))))(z ||= {});

```