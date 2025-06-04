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
import { __toDynamicImportESM } from "./chunk.js";

//#region a.js
x ? import("a") : y ? import("./import.js").then(__toDynamicImportESM()) : import("c");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,6 +1,2 @@
-var require_import = __commonJS({
-    "import.js"(exports) {
-        exports.foo = 213;
-    }
-});
-x ? import("a") : y ? Promise.resolve().then(() => __toESM(require_import())) : import("c");
+import {__toDynamicImportESM} from "./chunk.js";
+x ? import("a") : y ? import("./import.js").then(__toDynamicImportESM()) : import("c");

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
import { __toDynamicImportESM } from "./chunk.js";

//#region b.js
x ? y ? import("a") : import("./import.js").then(__toDynamicImportESM()) : import(c);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,6 +1,2 @@
-var require_import = __commonJS({
-    "import.js"(exports) {
-        exports.foo = 213;
-    }
-});
-x ? y ? import("a") : Promise.resolve().then(() => __toESM(require_import())) : import(c);
+import {__toDynamicImportESM} from "./chunk.js";
+x ? y ? import("a") : import("./import.js").then(__toDynamicImportESM()) : import(c);

```