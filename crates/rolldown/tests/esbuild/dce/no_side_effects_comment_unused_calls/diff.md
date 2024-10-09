# Diff
## /out/stmt-fn.js
### esbuild
```js
// @__NO_SIDE_EFFECTS__
function f(y) {
  sideEffect(y);
}
// @__NO_SIDE_EFFECTS__
function* g(y) {
  sideEffect(y);
}
onlyKeepThisIdentifier;
onlyKeepThisIdentifier;
x(/* @__PURE__ */ f("keepThisCall"));
x(/* @__PURE__ */ g("keepThisCall"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-fn.js
+++ rolldown	
@@ -1,10 +0,0 @@
-function f(y) {
-    sideEffect(y);
-}
-function* g(y) {
-    sideEffect(y);
-}
-onlyKeepThisIdentifier;
-onlyKeepThisIdentifier;
-x(f("keepThisCall"));
-x(g("keepThisCall"));

```
## /out/stmt-local.js
### esbuild
```js
const f = /* @__NO_SIDE_EFFECTS__ */ function(y) {
  sideEffect(y);
}, g = /* @__NO_SIDE_EFFECTS__ */ function* (y) {
  sideEffect(y);
};
onlyKeepThisIdentifier;
onlyKeepThisIdentifier;
x(/* @__PURE__ */ f("keepThisCall"));
x(/* @__PURE__ */ g("keepThisCall"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-local.js
+++ rolldown	
@@ -1,9 +0,0 @@
-const f = function (y) {
-    sideEffect(y);
-}, g = function* (y) {
-    sideEffect(y);
-};
-onlyKeepThisIdentifier;
-onlyKeepThisIdentifier;
-x(f("keepThisCall"));
-x(g("keepThisCall"));

```
## /out/expr-fn.js
### esbuild
```js
const f = /* @__NO_SIDE_EFFECTS__ */ function(y) {
  sideEffect(y);
}, g = /* @__NO_SIDE_EFFECTS__ */ function* (y) {
  sideEffect(y);
};
onlyKeepThisIdentifier;
onlyKeepThisIdentifier;
x(/* @__PURE__ */ f("keepThisCall"));
x(/* @__PURE__ */ g("keepThisCall"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/expr-fn.js
+++ rolldown	
@@ -1,9 +0,0 @@
-const f = function (y) {
-    sideEffect(y);
-}, g = function* (y) {
-    sideEffect(y);
-};
-onlyKeepThisIdentifier;
-onlyKeepThisIdentifier;
-x(f("keepThisCall"));
-x(g("keepThisCall"));

```
## /out/stmt-export-default-fn.js
### esbuild
```js
// @__NO_SIDE_EFFECTS__
export default function f(y) {
  sideEffect(y);
}
onlyKeepThisIdentifier;
x(/* @__PURE__ */ f("keepThisCall"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-fn.js
+++ rolldown	
@@ -1,5 +0,0 @@
-export default function f(y) {
-    sideEffect(y);
-}
-onlyKeepThisIdentifier;
-x(f("keepThisCall"));

```