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

```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	
@@ -1,4 +0,0 @@
-console.log([(void 0).factory("x", null), (void 0).factory("x", null)]);
-f = function () {
-    console.log([this.factory("y", null), this.factory("y", null)]);
-};

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

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	
@@ -1,3 +0,0 @@
-(console.log([(void 0).factory((void 0).fragment, null, "x"), (void 0).factory((void 0).fragment, null, "x")]), f = function () {
-    console.log([this.factory(this.fragment, null, "y"), this.factory(this.fragment, null, "y")]);
-});

```