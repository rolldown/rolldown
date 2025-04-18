# Reason
1. Could be done in minifier
# Diff
## /out/function2.js
### esbuild
```js
// function2.js
console.log(x());
function x() {
  return 2;
}
```
### rolldown
```js
//#region function2.js
function x() {
	return 1;
}
console.log(x());
function x() {
	return 2;
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/function2.js
+++ rolldown	function2.js
@@ -1,4 +1,10 @@
+//#region function2.js
+function x() {
+	return 1;
+}
 console.log(x());
 function x() {
-    return 2;
+	return 2;
 }
+
+//#endregion
\ No newline at end of file

```
## /out/function3.js
### esbuild
```js
// function3.js
console.log(x());
console.log(x());
function x() {
  return 3;
}
```
### rolldown
```js
//#region function3.js
function x() {
	return 1;
}
console.log(x());
function x() {
	return 2;
}
console.log(x());
function x() {
	return 3;
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/function3.js
+++ rolldown	function3.js
@@ -1,5 +1,14 @@
+//#region function3.js
+function x() {
+	return 1;
+}
 console.log(x());
+function x() {
+	return 2;
+}
 console.log(x());
 function x() {
-    return 3;
+	return 3;
 }
+
+//#endregion
\ No newline at end of file

```