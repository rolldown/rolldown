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

```
### diff
```diff
===================================================================
--- esbuild	/out/exported-entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var y_keep = 2;
-console.log(1, 2);
-export {y_keep};

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

```
### diff
```diff
===================================================================
--- esbuild	/out/re-exported-entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var y_keep = 2;
-console.log(1, 2);
-export {y_keep};

```
## /out/re-exported-2-entry.js
### esbuild
```js
// re-exported-2-constants.js
var y_keep = 2;
export {
  y_keep
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/re-exported-2-entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var y_keep = 2;
-export {y_keep};

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

```
### diff
```diff
===================================================================
--- esbuild	/out/re-exported-star-entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var x_keep = 1, y_keep = 2;
-export {x_keep, y_keep};

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

```
### diff
```diff
===================================================================
--- esbuild	/out/cross-module-entry.js
+++ rolldown	
@@ -1,6 +0,0 @@
-foo();
-var y_keep = 1;
-function foo() {
-    return [1, y_keep];
-}
-console.log(1, y_keep);

```
## /out/print-shorthand-entry.js
### esbuild
```js
// print-shorthand-entry.js
console.log({ foo: 123, a: -321 });
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/print-shorthand-entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-console.log({
-    foo: 123,
-    a: -321
-});

```
## /out/circular-import-entry.js
### esbuild
```js
// circular-import-cycle.js
console.log(bar());

// circular-import-constants.js
var foo = 123;
function bar() {
  return foo;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/circular-import-entry.js
+++ rolldown	
@@ -1,5 +0,0 @@
-console.log(bar());
-var foo = 123;
-function bar() {
-    return foo;
-}

```
## /out/circular-re-export-entry.js
### esbuild
```js
// circular-re-export-cycle.js
var baz = 0;
console.log(bar());

// circular-re-export-constants.js
var foo = 123;
function bar() {
  return foo;
}

// circular-re-export-entry.js
console.log(baz);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/circular-re-export-entry.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var baz = 0;
-console.log(bar());
-var foo = 123;
-function bar() {
-    return foo;
-}
-console.log(baz);

```
## /out/circular-re-export-star-entry.js
### esbuild
```js
// circular-re-export-star-cycle.js
console.log(bar());

// circular-re-export-star-constants.js
var foo = 123;
function bar() {
  return foo;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/circular-re-export-star-entry.js
+++ rolldown	
@@ -1,5 +0,0 @@
-console.log(bar());
-var foo = 123;
-function bar() {
-    return foo;
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/non-circular-export-entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function bar() {
-    return 123;
-}
-console.log(123, bar());

```