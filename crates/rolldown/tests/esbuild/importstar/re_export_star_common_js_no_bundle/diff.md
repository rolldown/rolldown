# Diff
## /out.js
### esbuild
```js
var entry_exports = {};
module.exports = __toCommonJS(entry_exports);
__reExport(entry_exports, require("foo"), module.exports);
```
### rolldown
```js
"use strict";
var foo = require("foo");
Object.keys(foo).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return foo[k]; }
  });
});
require("foo");


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,10 @@
-var entry_exports = {};
-module.exports = __toCommonJS(entry_exports);
-__reExport(entry_exports, require("foo"), module.exports);
+var foo = require("foo");
+Object.keys(foo).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return foo[k];
+        }
+    });
+});
+require("foo");

```