# Diff
## /out/identity.js
### esbuild
```js
// identity.js
console.log(1);
foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity.js
+++ rolldown	
@@ -1,2 +0,0 @@
-console.log(1);
-foo();

```
## /out/identity-last.js
### esbuild
```js
// identity-last.js
console.log(1);
foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-last.js
+++ rolldown	
@@ -1,2 +0,0 @@
-console.log(1);
-foo();

```
## /out/identity-first.js
### esbuild
```js
// identity-first.js
function keep(x) {
  return [x];
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-first.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep(x) {
-    return [x];
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/identity-generator.js
### esbuild
```js
// identity-generator.js
function* keep(x) {
  return x;
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-generator.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function* keep(x) {
-    return x;
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/identity-async.js
### esbuild
```js
// identity-async.js
async function keep(x) {
  return x;
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-async.js
+++ rolldown	
@@ -1,6 +0,0 @@
-async function keep(x) {
-    return x;
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/identity-cross-module.js
### esbuild
```js
// identity-cross-module.js
console.log(1);
foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-cross-module.js
+++ rolldown	
@@ -1,2 +0,0 @@
-console.log(1);
-foo();

```
## /out/identity-no-args.js
### esbuild
```js
// identity-no-args.js
function keep(x) {
  return x;
}
console.log(keep());
keep();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-no-args.js
+++ rolldown	
@@ -1,5 +0,0 @@
-function keep(x) {
-    return x;
-}
-console.log(keep());
-keep();

```
## /out/identity-two-args.js
### esbuild
```js
// identity-two-args.js
function keep(x) {
  return x;
}
console.log(keep(1, 2));
keep(1, 2);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-two-args.js
+++ rolldown	
@@ -1,5 +0,0 @@
-function keep(x) {
-    return x;
-}
-console.log(keep(1, 2));
-keep(1, 2);

```
## /out/reassign.js
### esbuild
```js
// reassign.js
function keep(x) {
  return x;
}
keep = reassigned;
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/reassign.js
+++ rolldown	
@@ -1,7 +0,0 @@
-function keep(x) {
-    return x;
-}
-keep = reassigned;
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/reassign-inc.js
### esbuild
```js
// reassign-inc.js
function keep(x) {
  return x;
}
keep++;
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/reassign-inc.js
+++ rolldown	
@@ -1,7 +0,0 @@
-function keep(x) {
-    return x;
-}
-keep++;
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/reassign-div.js
### esbuild
```js
// reassign-div.js
function keep(x) {
  return x;
}
keep /= reassigned;
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/reassign-div.js
+++ rolldown	
@@ -1,7 +0,0 @@
-function keep(x) {
-    return x;
-}
-keep /= reassigned;
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/reassign-array.js
### esbuild
```js
// reassign-array.js
function keep(x) {
  return x;
}
[keep] = reassigned;
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/reassign-array.js
+++ rolldown	
@@ -1,7 +0,0 @@
-function keep(x) {
-    return x;
-}
-[keep] = reassigned;
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/reassign-object.js
### esbuild
```js
// reassign-object.js
function keep(x) {
  return x;
}
({ keep } = reassigned);
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/reassign-object.js
+++ rolldown	
@@ -1,7 +0,0 @@
-function keep(x) {
-    return x;
-}
-({keep} = reassigned);
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/not-identity-two-args.js
### esbuild
```js
// not-identity-two-args.js
function keep(x, y) {
  return x;
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/not-identity-two-args.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep(x, y) {
-    return x;
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/not-identity-default.js
### esbuild
```js
// not-identity-default.js
function keep(x = foo()) {
  return x;
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/not-identity-default.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep(x = foo()) {
-    return x;
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/not-identity-array.js
### esbuild
```js
// not-identity-array.js
function keep([x]) {
  return x;
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/not-identity-array.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep([x]) {
-    return x;
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/not-identity-object.js
### esbuild
```js
// not-identity-object.js
function keep({ x }) {
  return x;
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/not-identity-object.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep({x}) {
-    return x;
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/not-identity-rest.js
### esbuild
```js
// not-identity-rest.js
function keep(...x) {
  return x;
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/not-identity-rest.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep(...x) {
-    return x;
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/not-identity-return.js
### esbuild
```js
// not-identity-return.js
function keep(x) {
  return [x];
}
console.log(keep(1));
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/not-identity-return.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep(x) {
-    return [x];
-}
-console.log(keep(1));
-keep(foo());
-keep(1);

```