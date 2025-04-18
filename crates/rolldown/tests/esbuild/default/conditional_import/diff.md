# Reason
1. esbuild will wrap `Promise.resolve().then() for original specifier`
# Diff
## /out/a.js
### esbuild
```js
// import.js
var require_import = __commonJS({
  "import.js"(exports) {
    exports.foo = 213;
  }
});

// a.js
x ? import("a") : y ? Promise.resolve().then(() => __toESM(require_import())) : import("c");
```
### rolldown
```js
//#region a.js
x ? import("a") : y ? import("./import.js") : import("c");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,6 +1,1 @@
-var require_import = __commonJS({
-    "import.js"(exports) {
-        exports.foo = 213;
-    }
-});
-x ? import("a") : y ? Promise.resolve().then(() => __toESM(require_import())) : import("c");
+x ? import("a") : y ? import("./import.js") : import("c");

```
## /out/b.js
### esbuild
```js
// import.js
var require_import = __commonJS({
  "import.js"(exports) {
    exports.foo = 213;
  }
});

// b.js
x ? y ? import("a") : Promise.resolve().then(() => __toESM(require_import())) : import(c);
```
### rolldown
```js
//#region b.js
x ? y ? import("a") : import("./import.js") : import(c);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,6 +1,1 @@
-var require_import = __commonJS({
-    "import.js"(exports) {
-        exports.foo = 213;
-    }
-});
-x ? y ? import("a") : Promise.resolve().then(() => __toESM(require_import())) : import(c);
+x ? y ? import("a") : import("./import.js") : import(c);

```