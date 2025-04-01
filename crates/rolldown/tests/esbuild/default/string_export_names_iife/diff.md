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
(function(exports, foo) {

"use strict";


foo = __toESM(foo);

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
exports["some export"] = foo["some import"]
return exports;
})({}, foo);
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,11 @@
-var global;
-(global ||= {}).name = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        "all the stuff": () => all_the_stuff,
-        "some export": () => import_foo["some import"]
+(function (exports, foo) {
+    foo = __toESM(foo);
+    Object.defineProperty(exports, 'all the stuff', {
+        enumerable: true,
+        get: function () {
+            return foo;
+        }
     });
-    var import_foo = require("./foo");
-    var all_the_stuff = __toESM(require("./foo"));
-    return __toCommonJS(entry_exports);
-})();
+    exports["some export"] = foo["some import"];
+    return exports;
+})({}, foo);

```