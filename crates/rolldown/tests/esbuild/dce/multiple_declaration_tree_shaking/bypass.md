# Diff
## /out/function2.js
### esbuild
```js
// function2.js
function x() {
  return 1;
}
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

```
### diff
```diff
===================================================================
--- esbuild	/out/function2.js
+++ rolldown	function2.js
@@ -1,8 +1,9 @@
-// function2.js
+
+//#region function2.js
 function x() {
-  return 1;
+	return 1;
 }
 console.log(x());
 function x() {
-  return 2;
-}
\ No newline at end of file
+	return 2;
+}

```
## /out/function3.js
### esbuild
```js
// function3.js
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

```
### diff
```diff
===================================================================
--- esbuild	/out/function3.js
+++ rolldown	function3.js
@@ -1,12 +1,13 @@
-// function3.js
+
+//#region function3.js
 function x() {
-  return 1;
+	return 1;
 }
 console.log(x());
 function x() {
-  return 2;
+	return 2;
 }
 console.log(x());
 function x() {
-  return 3;
-}
\ No newline at end of file
+	return 3;
+}

```