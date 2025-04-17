# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(entry_exports);
__reExport(entry_exports, require("fs"), module.exports);

// internal.js
var foo = 123;

// entry.js
__reExport(entry_exports, require("./external"), module.exports);
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  foo,
  ...require("fs"),
  ...require("./external")
});
```
### rolldown
```js
"use strict";

//#region internal.js
let foo = 123;

exports.foo = foo
var fs = require("fs");
Object.keys(fs).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return fs[k]; }
  });
});

var external = require("./external");
Object.keys(external).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return external[k]; }
  });
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,13 +1,20 @@
-var entry_exports = {};
-__export(entry_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(entry_exports);
-__reExport(entry_exports, require("fs"), module.exports);
 var foo = 123;
-__reExport(entry_exports, require("./external"), module.exports);
-0 && (module.exports = {
-    foo,
-    ...require("fs"),
-    ...require("./external")
+exports.foo = foo;
+var fs = require("fs");
+Object.keys(fs).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return fs[k];
+        }
+    });
 });
+var external = require("./external");
+Object.keys(external).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return external[k];
+        }
+    });
+});

```