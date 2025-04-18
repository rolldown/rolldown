# Reason
1. no sideEffect comment detect
# Diff
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
//#region stmt-local.js
const f = /* @__NO_SIDE_EFFECTS__ */ function(y) {
	sideEffect(y);
};
const g = /* @__NO_SIDE_EFFECTS__ */ function* (y) {
	sideEffect(y);
};
onlyKeepThisIdentifier;
onlyKeepThisIdentifier;
x(/* @__PURE__ */ f("keepThisCall"));
x(/* @__PURE__ */ g("keepThisCall"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-local.js
+++ rolldown	stmt-local.js
@@ -1,7 +1,8 @@
-const f = function (y) {
+var f = function (y) {
     sideEffect(y);
-}, g = function* (y) {
+};
+var g = function* (y) {
     sideEffect(y);
 };
 onlyKeepThisIdentifier;
 onlyKeepThisIdentifier;

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
//#region expr-fn.js
const f = /* @__NO_SIDE_EFFECTS__ */ function(y) {
	sideEffect(y);
};
const g = /* @__NO_SIDE_EFFECTS__ */ function* (y) {
	sideEffect(y);
};
onlyKeepThisIdentifier;
onlyKeepThisIdentifier;
x(/* @__PURE__ */ f("keepThisCall"));
x(/* @__PURE__ */ g("keepThisCall"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/expr-fn.js
+++ rolldown	expr-fn.js
@@ -1,7 +1,8 @@
-const f = function (y) {
+var f = function (y) {
     sideEffect(y);
-}, g = function* (y) {
+};
+var g = function* (y) {
     sideEffect(y);
 };
 onlyKeepThisIdentifier;
 onlyKeepThisIdentifier;

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
//#region stmt-export-default-fn.js
/* @__NO_SIDE_EFFECTS__ */
function f(y) {
	sideEffect(y);
}
onlyKeepThisIdentifier;
x(/* @__PURE__ */ f("keepThisCall"));

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-fn.js
+++ rolldown	stmt-export-default-fn.js
@@ -1,5 +1,6 @@
-export default function f(y) {
+function f(y) {
     sideEffect(y);
 }
 onlyKeepThisIdentifier;
 x(f("keepThisCall"));
+export {f as default};

```