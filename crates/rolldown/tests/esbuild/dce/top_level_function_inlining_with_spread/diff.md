# Diff
## /out/entry.js
### esbuild
```js
// entry.js
function identity1(x) {
  return x;
}
function identity3(x) {
  return x;
}
args;
[...args];
identity1();
args;
identity3(...args);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,11 +0,0 @@
-function identity1(x) {
-    return x;
-}
-function identity3(x) {
-    return x;
-}
-args;
-[...args];
-identity1();
-args;
-identity3(...args);

```
## /out/entry-outer.js
### esbuild
```js
// inner.js
function identity1(x) {
  return x;
}
function identity3(x) {
  return x;
}

// entry-outer.js
args;
[...args];
identity1();
args;
identity3(...args);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-outer.js
+++ rolldown	
@@ -1,11 +0,0 @@
-function identity1(x) {
-    return x;
-}
-function identity3(x) {
-    return x;
-}
-args;
-[...args];
-identity1();
-args;
-identity3(...args);

```