# Reason
1. `jsx.factory`
# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
console.log([
  /* @__PURE__ */ (void 0).factory("x", null),
  /* @__PURE__ */ (void 0).factory("x", null)
]);
f = function() {
  console.log([
    /* @__PURE__ */ this.factory("y", null),
    /* @__PURE__ */ this.factory("y", null)
  ]);
};
```
### rolldown
```js

//#region factory.jsx
console.log([this.factory("x", null), /* @__PURE__ */ this.factory("x", null)]);
f = function() {
	console.log([this.factory("y", null), /* @__PURE__ */ this.factory("y", null)]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,4 +1,4 @@
-console.log([(void 0).factory("x", null), (void 0).factory("x", null)]);
+console.log([this.factory("x", null), this.factory("x", null)]);
 f = function () {
     console.log([this.factory("y", null), this.factory("y", null)]);
 };

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
console.log([
  /* @__PURE__ */ (void 0).factory((void 0).fragment, null, "x"),
  /* @__PURE__ */ (void 0).factory((void 0).fragment, null, "x")
]), f = function() {
  console.log([
    /* @__PURE__ */ this.factory(this.fragment, null, "y"),
    /* @__PURE__ */ this.factory(this.fragment, null, "y")
  ]);
};
```
### rolldown
```js

//#region fragment.jsx
console.log([this.factory(this.fragment, null, "x"), /* @__PURE__ */ this.factory(this.fragment, null, "x")]), f = function() {
	console.log([this.factory(this.fragment, null, "y"), /* @__PURE__ */ this.factory(this.fragment, null, "y")]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,3 +1,3 @@
-(console.log([(void 0).factory((void 0).fragment, null, "x"), (void 0).factory((void 0).fragment, null, "x")]), f = function () {
+(console.log([this.factory(this.fragment, null, "x"), this.factory(this.fragment, null, "x")]), f = function () {
     console.log([this.factory(this.fragment, null, "y"), this.factory(this.fragment, null, "y")]);
 });

```