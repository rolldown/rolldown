# Reason
1. rolldown has redundant `require('external')`
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  inner: () => inner_exports
});
module.exports = __toCommonJS(entry_exports);
__reExport(entry_exports, require("a"), module.exports);

// inner.js
var inner_exports = {};
__reExport(inner_exports, require("b"));
```
### rolldown
```js
"use strict";
var a$1 = require("a");
Object.keys(a$1).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return a$1[k]; }
  });
});

require("a");
require("b");

//#region inner.js
var inner_exports = {};
__reExport(inner_exports, require("b"));

//#endregion
Object.defineProperty(exports, 'inner', {
  enumerable: true,
  get: function () {
    return inner_exports;
  }
});
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,8 +1,19 @@
-var entry_exports = {};
-__export(entry_exports, {
-    inner: () => inner_exports
+var a$1 = require("a");
+Object.keys(a$1).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return a$1[k];
+        }
+    });
 });
-module.exports = __toCommonJS(entry_exports);
-__reExport(entry_exports, require("a"), module.exports);
+require("a");
+require("b");
 var inner_exports = {};
 __reExport(inner_exports, require("b"));
+Object.defineProperty(exports, 'inner', {
+    enumerable: true,
+    get: function () {
+        return inner_exports;
+    }
+});

```