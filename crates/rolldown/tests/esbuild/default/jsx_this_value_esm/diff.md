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

```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	
@@ -1,4 +0,0 @@
-console.log([(void 0)("x", null), (void 0)("x", null)]);
-f = function () {
-    console.log([this("y", null), this("y", null)]);
-};

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

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	
@@ -1,3 +0,0 @@
-(console.log([(void 0)(void 0, null, "x"), (void 0)(void 0, null, "x")]), f = function () {
-    console.log([this(this, null, "y"), this(this, null, "y")]);
-});

```