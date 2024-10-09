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

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_import = __commonJS({
-    "import.js"(exports) {
-        exports.foo = 213;
-    }
-});
-x ? import("a") : y ? Promise.resolve().then(() => __toESM(require_import())) : import("c");

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

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_import = __commonJS({
-    "import.js"(exports) {
-        exports.foo = 213;
-    }
-});
-x ? y ? import("a") : Promise.resolve().then(() => __toESM(require_import())) : import(c);

```