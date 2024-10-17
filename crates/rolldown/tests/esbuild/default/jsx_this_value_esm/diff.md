# Reason
1. `jsx.factory`
# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
console.log([
  /* @__PURE__ */ (void 0)("x", null),
  /* @__PURE__ */ (void 0)("x", null)
]);
f = function() {
  console.log([
    /* @__PURE__ */ this("y", null),
    /* @__PURE__ */ this("y", null)
  ]);
};
```
### rolldown
```js

//#region factory.jsx
console.log([this("x", null), /* @__PURE__ */ this("x", null)]);
f = function() {
	console.log([this("y", null), /* @__PURE__ */ this("y", null)]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,4 +1,4 @@
-console.log([(void 0)("x", null), (void 0)("x", null)]);
+console.log([this("x", null), this("x", null)]);
 f = function () {
     console.log([this("y", null), this("y", null)]);
 };

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
console.log([
  /* @__PURE__ */ (void 0)(void 0, null, "x"),
  /* @__PURE__ */ (void 0)(void 0, null, "x")
]), f = function() {
  console.log([
    /* @__PURE__ */ this(this, null, "y"),
    /* @__PURE__ */ this(this, null, "y")
  ]);
};
```
### rolldown
```js

//#region fragment.jsx
console.log([this(this, null, "x"), /* @__PURE__ */ this(this, null, "x")]), f = function() {
	console.log([this(this, null, "y"), /* @__PURE__ */ this(this, null, "y")]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,3 +1,3 @@
-(console.log([(void 0)(void 0, null, "x"), (void 0)(void 0, null, "x")]), f = function () {
+(console.log([this(this, null, "x"), this(this, null, "x")]), f = function () {
     console.log([this(this, null, "y"), this(this, null, "y")]);
 });

```