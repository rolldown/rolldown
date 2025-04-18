# Reason
1. could be done in minifier
# Diff
## /out/top-level-no-eval.js
### esbuild
```js
const x = 1;
console.log(1, evil("x"));
```
### rolldown
```js
//#region top-level-no-eval.js
const x = 1;
console.log(x, evil("x"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/top-level-no-eval.js
+++ rolldown	top-level-no-eval.js
@@ -1,2 +1,2 @@
-const x = 1;
-console.log(1, evil("x"));
+var x = 1;
+console.log(x, evil("x"));

```
## /out/top-level-eval.js
### esbuild
```js
const x = 1;
console.log(1, eval("x"));
```
### rolldown
```js
//#region top-level-eval.js
const x = 1;
console.log(x, eval("x"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/top-level-eval.js
+++ rolldown	top-level-eval.js
@@ -1,2 +1,2 @@
-const x = 1;
-console.log(1, eval("x"));
+var x = 1;
+console.log(x, eval("x"));

```
## /out/nested-no-eval.js
### esbuild
```js
console.log(1, evil("x"));
```
### rolldown
```js
//#region nested-no-eval.js
(() => {
	const x = 1;
	console.log(x, evil("x"));
})();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-no-eval.js
+++ rolldown	nested-no-eval.js
@@ -1,1 +1,4 @@
-console.log(1, evil("x"));
+(() => {
+    const x = 1;
+    console.log(x, evil("x"));
+})();

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
//#region nested-eval.js
(() => {
	const x = 1;
	console.log(x, eval("x"));
})();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-eval.js
+++ rolldown	nested-eval.js
@@ -1,4 +1,4 @@
 (() => {
     const x = 1;
-    console.log(1, eval("x"));
+    console.log(x, eval("x"));
 })();

```
## /out/ts-namespace-no-eval.js
### esbuild
```js
var y;
((y2) => (y2.x = 1, console.log(1, evil("x"))))(y ||= {});
```
### rolldown
```js
//#region ts-namespace-no-eval.ts
let y;
(function(_y) {
	const x = _y.x = 1;
	console.log(x, evil("x"));
})(y || (y = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/ts-namespace-no-eval.js
+++ rolldown	ts-namespace-no-eval.js
@@ -1,2 +1,5 @@
 var y;
-(y2 => (y2.x = 1, console.log(1, evil("x"))))(y ||= {});
+(function (_y) {
+    const x = _y.x = 1;
+    console.log(x, evil("x"));
+})(y || (y = {}));

```
## /out/ts-namespace-eval.js
### esbuild
```js
var z;
((z) => (z.x = 1, console.log(1, eval("x"))))(z ||= {});
```
### rolldown
```js
//#region ts-namespace-eval.ts
let z;
(function(_z) {
	const x = _z.x = 1;
	console.log(x, eval("x"));
})(z || (z = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/ts-namespace-eval.js
+++ rolldown	ts-namespace-eval.js
@@ -1,2 +1,5 @@
 var z;
-(z => (z.x = 1, console.log(1, eval("x"))))(z ||= {});
+(function (_z) {
+    const x = _z.x = 1;
+    console.log(x, eval("x"));
+})(z || (z = {}));

```