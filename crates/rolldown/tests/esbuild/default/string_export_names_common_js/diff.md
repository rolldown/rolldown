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

const all the stuff = __toESM(require("./foo"));
const { some import: someImport } = __toESM(require("./foo"));

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return all the stuff;
  }
});
exports["some export"] = someImport
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,12 @@
-var entry_exports = {};
-__export(entry_exports, {
-    "all the stuff": () => all_the_stuff,
-    "some export": () => import_foo["some import"]
+"use strict";
+
+const all the stuff = __toESM(require("./foo"));
+const { some import: someImport } = __toESM(require("./foo"));
+
+Object.defineProperty(exports, 'all the stuff', {
+  enumerable: true,
+  get: function () {
+    return all the stuff;
+  }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = require("./foo");
-var all_the_stuff = __toESM(require("./foo"));
+exports["some export"] = someImport
\ No newline at end of file

```