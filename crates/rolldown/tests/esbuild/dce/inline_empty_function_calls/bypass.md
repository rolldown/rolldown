# Reason
1. could be done in minifier
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

//#region empty.js
function DROP() {}
console.log(DROP(foo(), bar()));
console.log(DROP(foo(), 1));
console.log(DROP(1, foo()));
console.log(DROP(1));
console.log(DROP());
DROP(foo(), bar());
DROP(foo(), 1);
DROP(1, foo());
DROP(1);
DROP();

```
### diff
```diff
===================================================================
--- esbuild	/out/empty.js
+++ rolldown	empty.js
@@ -1,8 +1,11 @@
-console.log((foo(), bar(), void 0));
-console.log((foo(), void 0));
-console.log((foo(), void 0));
-console.log(void 0);
-console.log(void 0);
-(foo(), bar());
-foo();
-foo();
+function DROP() {}
+console.log(DROP(foo(), bar()));
+console.log(DROP(foo(), 1));
+console.log(DROP(1, foo()));
+console.log(DROP(1));
+console.log(DROP());
+DROP(foo(), bar());
+DROP(foo(), 1);
+DROP(1, foo());
+DROP(1);
+DROP();

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

//#region empty-comma.js
function DROP() {}
console.log((DROP(), DROP(), foo()));
console.log((DROP(), foo(), DROP()));
console.log((foo(), DROP(), DROP()));
for (DROP(); DROP(); DROP()) DROP();
DROP(), DROP(), foo();
DROP(), foo(), DROP();
foo(), DROP(), DROP();

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-comma.js
+++ rolldown	empty-comma.js
@@ -1,7 +1,8 @@
-console.log(foo());
-console.log((foo(), void 0));
-console.log((foo(), void 0));
-for (; void 0; ) ;
-foo();
-foo();
-foo();
+function DROP() {}
+console.log((DROP(), DROP(), foo()));
+console.log((DROP(), foo(), DROP()));
+console.log((foo(), DROP(), DROP()));
+for (DROP(); DROP(); DROP()) DROP();
+(DROP(), DROP(), foo());
+(DROP(), foo(), DROP());
+(foo(), DROP(), DROP());

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

//#region empty-if-else.js
function DROP() {}
if (foo) {
	let bar = baz();
	bar();
	bar();
} else DROP();

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-if-else.js
+++ rolldown	empty-if-else.js
@@ -1,4 +1,6 @@
+function DROP() {}
 if (foo) {
     let bar = baz();
-    (bar(), bar());
-}
+    bar();
+    bar();
+} else DROP();

```
## /out/empty-last.js
### esbuild
```js
// empty-last.js
console.log(void 0);
```
### rolldown
```js

//#region empty-last.js
function DROP() {
	return x;
}
function DROP() {
	return;
}
console.log(DROP());
DROP();

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-last.js
+++ rolldown	empty-last.js
@@ -1,1 +1,10 @@
-console.log(void 0);
+
+//#region empty-last.js
+function DROP() {
+	return x;
+}
+function DROP() {
+	return;
+}
+console.log(DROP());
+DROP();

```
## /out/empty-cross-module.js
### esbuild
```js
// empty-cross-module.js
console.log(void 0);
```
### rolldown
```js

//#region empty-cross-module-def.js
function DROP() {}

//#region empty-cross-module.js
console.log(DROP());
DROP();

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-cross-module.js
+++ rolldown	empty-cross-module.js
@@ -1,1 +1,3 @@
-console.log(void 0);
+function DROP() {}
+console.log(DROP());
+DROP();

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

//#region empty-first.js
function keep() {
	return;
}
function keep() {
	return x;
}
console.log(keep());
keep(foo());
keep(1);

```
### diff
```diff
===================================================================
--- esbuild	/out/empty-first.js
+++ rolldown	empty-first.js
@@ -1,6 +1,11 @@
+
+//#region empty-first.js
 function keep() {
-    return x;
+	return;
 }
+function keep() {
+	return x;
+}
 console.log(keep());
 keep(foo());
 keep(1);

```