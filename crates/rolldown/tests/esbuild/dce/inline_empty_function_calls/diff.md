# Diff
## /out/empty.js
### esbuild
```js
// empty.js
console.log((foo(), bar(), void 0));
console.log((foo(), void 0));
console.log((foo(), void 0));
console.log(void 0);
console.log(void 0);
foo(), bar();
foo();
foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty.js
+++ rolldown	
@@ -1,8 +0,0 @@
-console.log((foo(), bar(), void 0));
-console.log((foo(), void 0));
-console.log((foo(), void 0));
-console.log(void 0);
-console.log(void 0);
-(foo(), bar());
-foo();
-foo();

```
## /out/empty-comma.js
### esbuild
```js
// empty-comma.js
console.log(foo());
console.log((foo(), void 0));
console.log((foo(), void 0));
for (; void 0; ) ;
foo();
foo();
foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-comma.js
+++ rolldown	
@@ -1,7 +0,0 @@
-console.log(foo());
-console.log((foo(), void 0));
-console.log((foo(), void 0));
-for (; void 0; ) ;
-foo();
-foo();
-foo();

```
## /out/empty-if-else.js
### esbuild
```js
// empty-if-else.js
if (foo) {
  let bar = baz();
  bar(), bar();
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-if-else.js
+++ rolldown	
@@ -1,4 +0,0 @@
-if (foo) {
-    let bar = baz();
-    (bar(), bar());
-}

```
## /out/empty-last.js
### esbuild
```js
// empty-last.js
console.log(void 0);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-last.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(void 0);

```
## /out/empty-cross-module.js
### esbuild
```js
// empty-cross-module.js
console.log(void 0);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-cross-module.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(void 0);

```
## /out/empty-first.js
### esbuild
```js
// empty-first.js
function keep() {
  return x;
}
console.log(keep());
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-first.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function keep() {
-    return x;
-}
-console.log(keep());
-keep(foo());
-keep(1);

```
## /out/empty-generator.js
### esbuild
```js
// empty-generator.js
function* keep() {
}
console.log(keep());
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-generator.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function* keep() {}
-console.log(keep());
-keep(foo());
-keep(1);

```
## /out/empty-async.js
### esbuild
```js
// empty-async.js
async function keep() {
}
console.log(keep());
keep(foo());
keep(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-async.js
+++ rolldown	
@@ -1,4 +0,0 @@
-async function keep() {}
-console.log(keep());
-keep(foo());
-keep(1);

```
## /out/reassign.js
### esbuild
```js
// reassign.js
function keep() {
}
keep = reassigned;
console.log(keep());
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
@@ -1,5 +0,0 @@
-function keep() {}
-keep = reassigned;
-console.log(keep());
-keep(foo());
-keep(1);

```
## /out/reassign-inc.js
### esbuild
```js
// reassign-inc.js
function keep() {
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
@@ -1,5 +0,0 @@
-function keep() {}
-keep++;
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/reassign-div.js
### esbuild
```js
// reassign-div.js
function keep() {
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
@@ -1,5 +0,0 @@
-function keep() {}
-keep /= reassigned;
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/reassign-array.js
### esbuild
```js
// reassign-array.js
function keep() {
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
@@ -1,5 +0,0 @@
-function keep() {}
-[keep] = reassigned;
-console.log(keep(1));
-keep(foo());
-keep(1);

```
## /out/reassign-object.js
### esbuild
```js
// reassign-object.js
function keep() {
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
@@ -1,5 +0,0 @@
-function keep() {}
-({keep} = reassigned);
-console.log(keep(1));
-keep(foo());
-keep(1);

```