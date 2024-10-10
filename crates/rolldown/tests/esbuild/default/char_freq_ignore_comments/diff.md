# Diff
## /out/a.js
### esbuild
```js
// a.js
function u(e, t, n, r) {
  return "the argument names must be the same";
}
export {
  u as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function u(e, t, n, r) {
-    return "the argument names must be the same";
-}
-export {u as default};

```
## /out/b.js
### esbuild
```js
// b.js
function u(e, t, n, r) {
  return "the argument names must be the same";
}
export {
  u as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function u(e, t, n, r) {
-    return "the argument names must be the same";
-}
-export {u as default};

```