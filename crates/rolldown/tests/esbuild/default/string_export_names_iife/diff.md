# Reason
1. string export name not correct
# Diff
## /out.js
### esbuild
```js
var global;
(global ||= {}).name = (() => {
  var entry_exports = {};
  __export(entry_exports, {
    "all the stuff": () => all_the_stuff,
    "some export": () => import_foo["some import"]
  });
  var import_foo = require("./foo");
  var all_the_stuff = __toESM(require("./foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
(function(exports, ___foo, ___foo) {

"use strict";
const all the stuff = ___foo;
const { some import: someImport } = ___foo;

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return all the stuff;
  }
});
exports["some export"] = someImport
return exports;
})({}, ___foo, ___foo);
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,15 @@
-var global;
-(global ||= {}).name = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        "all the stuff": () => all_the_stuff,
-        "some export": () => import_foo["some import"]
-    });
-    var import_foo = require("./foo");
-    var all_the_stuff = __toESM(require("./foo"));
-    return __toCommonJS(entry_exports);
-})();
+(function(exports, ___foo, ___foo) {
+
+"use strict";
+const all the stuff = ___foo;
+const { some import: someImport } = ___foo;
+
+Object.defineProperty(exports, 'all the stuff', {
+  enumerable: true,
+  get: function () {
+    return all the stuff;
+  }
+});
+exports["some export"] = someImport
+return exports;
+})({}, ___foo, ___foo);
\ No newline at end of file

```