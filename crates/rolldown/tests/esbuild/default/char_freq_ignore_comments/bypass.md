# Reason
1. could be done in minifier
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

//#region a.js
function a_default(one, two, three, four) {
	return "the argument names must be the same";
}

export { a_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,4 +1,4 @@
-function u(e, t, n, r) {
+function a_default(one, two, three, four) {
     return "the argument names must be the same";
 }
-export {u as default};
+export {a_default as default};

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

//#region b.js
function b_default(one, two, three, four) {
	return "the argument names must be the same";
}

export { b_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,4 +1,4 @@
-function u(e, t, n, r) {
+function b_default(one, two, three, four) {
     return "the argument names must be the same";
 }
-export {u as default};
+export {b_default as default};

```