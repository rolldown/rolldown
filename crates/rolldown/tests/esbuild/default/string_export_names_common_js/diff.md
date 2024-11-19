# Reason
1. obviously, the output is incorrect
# Diff
## /out.js
### esbuild
```js
var entry_exports = {};
__export(entry_exports, {
  "all the stuff": () => all_the_stuff,
  "some export": () => import_foo["some import"]
});
module.exports = __toCommonJS(entry_exports);
var import_foo = require("./foo");
var all_the_stuff = __toESM(require("./foo"));
```
### rolldown
```js
"use strict";

const ___foo = __toESM(require("./foo"));

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return ___foo;
  }
});
exports["some export"] = ___foo["some import"]
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
-var entry_exports = {};
-__export(entry_exports, {
-    "all the stuff": () => all_the_stuff,
-    "some export": () => import_foo["some import"]
+var ___foo = __toESM(require("./foo"));
+Object.defineProperty(exports, 'all the stuff', {
+    enumerable: true,
+    get: function () {
+        return ___foo;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = require("./foo");
-var all_the_stuff = __toESM(require("./foo"));
+exports["some export"] = ___foo["some import"];

```