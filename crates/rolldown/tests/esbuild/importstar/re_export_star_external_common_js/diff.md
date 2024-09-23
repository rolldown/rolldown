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
+++ rolldown	entry_js.cjs
@@ -1,3 +1,11 @@
-var entry_exports = {};
-module.exports = __toCommonJS(entry_exports);
-__reExport(entry_exports, require('foo'), module.exports);
\ No newline at end of file
+var foo = require('foo');
+Object.keys(foo).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k))
+        Object.defineProperty(exports, k, {
+            enumerable: true,
+            get: function () {
+                return foo[k];
+            }
+        });
+});
+require('foo');
\ No newline at end of file

```
