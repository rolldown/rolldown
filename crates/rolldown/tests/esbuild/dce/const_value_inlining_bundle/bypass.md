# Reason
1. could be done in minifier
# Diff
## /out/exported-entry.js
### esbuild
```js
// exported-entry.js
var y_keep = 2;
console.log(
  1,
  2
);
export {
  y_keep
};
```
### rolldown
```js

//#region exported-entry.js
const x_REMOVE = 1;
const y_keep = 2;
console.log(x_REMOVE, y_keep);
//#endregion

export { y_keep };
```
### diff
```diff
===================================================================
--- esbuild	/out/exported-entry.js
+++ rolldown	exported-entry.js
@@ -1,3 +1,4 @@
+var x_REMOVE = 1;
 var y_keep = 2;
-console.log(1, 2);
+console.log(x_REMOVE, y_keep);
 export {y_keep};

```
## /out/re-exported-entry.js
### esbuild
```js
// re-exported-constants.js
var y_keep = 2;

// re-exported-entry.js
console.log(1, 2);
export {
  y_keep
};
```
### rolldown
```js

//#region re-exported-constants.js
const x_REMOVE = 1;
const y_keep = 2;
//#endregion

//#region re-exported-entry.js
console.log(x_REMOVE, y_keep);
//#endregion

export { y_keep };
```
### diff
```diff
===================================================================
--- esbuild	/out/re-exported-entry.js
+++ rolldown	re-exported-entry.js
@@ -1,3 +1,4 @@
+var x_REMOVE = 1;
 var y_keep = 2;
-console.log(1, 2);
+console.log(x_REMOVE, y_keep);
 export {y_keep};

```
## /out/re-exported-star-entry.js
### esbuild
```js
// re-exported-star-constants.js
var x_keep = 1, y_keep = 2;
export {
  x_keep,
  y_keep
};
```
### rolldown
```js

//#region re-exported-star-constants.js
const x_keep = 1;
const y_keep = 2;
//#endregion

export { x_keep, y_keep };
```
### diff
```diff
===================================================================
--- esbuild	/out/re-exported-star-entry.js
+++ rolldown	re-exported-star-entry.js
@@ -1,2 +1,3 @@
-var x_keep = 1, y_keep = 2;
+var x_keep = 1;
+var y_keep = 2;
 export {x_keep, y_keep};

```
## /out/cross-module-entry.js
### esbuild
```js
// cross-module-constants.js
foo();
var y_keep = 1;
function foo() {
  return [1, y_keep];
}

// cross-module-entry.js
console.log(1, y_keep);
```
### rolldown
```js

//#region cross-module-constants.js
const x_REMOVE = 1;
foo();
const y_keep = 1;
function foo() {
	return [x_REMOVE, y_keep];
}
//#endregion

//#region cross-module-entry.js
console.log(x_REMOVE, y_keep);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/cross-module-entry.js
+++ rolldown	cross-module-entry.js
@@ -1,6 +1,7 @@
+var x_REMOVE = 1;
 foo();
 var y_keep = 1;
 function foo() {
-    return [1, y_keep];
+    return [x_REMOVE, y_keep];
 }
-console.log(1, y_keep);
+console.log(x_REMOVE, y_keep);

```
## /out/print-shorthand-entry.js
### esbuild
```js
// print-shorthand-entry.js
console.log({ foo: 123, a: -321 });
```
### rolldown
```js

//#region print-shorthand-constants.js
const foo = 123;
const _bar = -321;
//#endregion

//#region print-shorthand-entry.js
console.log({
	foo,
	_bar
});
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/print-shorthand-entry.js
+++ rolldown	print-shorthand-entry.js
@@ -1,4 +1,6 @@
+var foo = 123;
+var _bar = -321;
 console.log({
-    foo: 123,
-    a: -321
+    foo,
+    _bar
 });

```
## /out/non-circular-export-entry.js
### esbuild
```js
// non-circular-export-constants.js
function bar() {
  return 123;
}

// non-circular-export-entry.js
console.log(123, bar());
```
### rolldown
```js

//#region non-circular-export-constants.js
const foo = 123;
function bar() {
	return foo;
}
//#endregion

//#region non-circular-export-entry.js
console.log(foo, bar());
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/non-circular-export-entry.js
+++ rolldown	non-circular-export-entry.js
@@ -1,4 +1,5 @@
+var foo = 123;
 function bar() {
-    return 123;
+    return foo;
 }
-console.log(123, bar());
+console.log(foo, bar());

```