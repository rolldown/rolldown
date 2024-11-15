# Reason
1. rolldown has redundant `require('external')`
# Diff
## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
module.exports = __toCommonJS(entry_exports);
__reExport(entry_exports, require("foo"), module.exports);
```
### rolldown
```js
"use strict";
var foo$1 = require("foo");
Object.keys(foo$1).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return foo$1[k]; }
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
+var foo$1 = require("foo");
+Object.keys(foo$1).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return foo$1[k];
+        }
+    });
+});
+require("foo");

```