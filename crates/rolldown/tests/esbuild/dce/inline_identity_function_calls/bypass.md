# Reason
1. could be done in minifier
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

//#region identity.js
function DROP(x) {
	return x;
}
console.log(DROP(1));
DROP(foo());
DROP(1);

```
### diff
```diff
===================================================================
--- esbuild	/out/identity.js
+++ rolldown	identity.js
@@ -1,2 +1,6 @@
-console.log(1);
-foo();
+function DROP(x) {
+    return x;
+}
+console.log(DROP(1));
+DROP(foo());
+DROP(1);

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

//#region identity-last.js
function DROP(x) {
	return [x];
}
function DROP(x) {
	return x;
}
console.log(DROP(1));
DROP(foo());
DROP(1);

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-last.js
+++ rolldown	identity-last.js
@@ -1,2 +1,11 @@
-console.log(1);
-foo();
+
+//#region identity-last.js
+function DROP(x) {
+	return [x];
+}
+function DROP(x) {
+	return x;
+}
+console.log(DROP(1));
+DROP(foo());
+DROP(1);

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

//#region identity-first.js
function keep(x) {
	return x;
}
function keep(x) {
	return [x];
}
console.log(keep(1));
keep(foo());
keep(1);

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-first.js
+++ rolldown	identity-first.js
@@ -1,6 +1,11 @@
+
+//#region identity-first.js
 function keep(x) {
-    return [x];
+	return x;
 }
+function keep(x) {
+	return [x];
+}
 console.log(keep(1));
 keep(foo());
 keep(1);

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

//#region identity-cross-module-def.js
function DROP(x) {
	return x;
}

//#region identity-cross-module.js
console.log(DROP(1));
DROP(foo());
DROP(1);

```
### diff
```diff
===================================================================
--- esbuild	/out/identity-cross-module.js
+++ rolldown	identity-cross-module.js
@@ -1,2 +1,6 @@
-console.log(1);
-foo();
+function DROP(x) {
+    return x;
+}
+console.log(DROP(1));
+DROP(foo());
+DROP(1);

```