# Reason
1. should not reuse `__toESM(require('./foo'))`
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
// HIDDEN [rolldown:runtime]
const foo = __toESM(require("./foo"));

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
exports["some export"] = foo["some import"];
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
+var foo = __toESM(require("./foo"));
+Object.defineProperty(exports, 'all the stuff', {
+    enumerable: true,
+    get: function () {
+        return foo;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = require("./foo");
-var all_the_stuff = __toESM(require("./foo"));
+exports["some export"] = foo["some import"];

```