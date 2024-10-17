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
(function(exports, __foo, __foo) {

"use strict";
const all the stuff = __foo;
const { some import: someImport } = __foo;

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return all the stuff;
  }
});
exports["some export"] = someImport
return exports;
})({}, __foo, __foo);
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
+(function(exports, __foo, __foo) {
+
+"use strict";
+const all the stuff = __foo;
+const { some import: someImport } = __foo;
+
+Object.defineProperty(exports, 'all the stuff', {
+  enumerable: true,
+  get: function () {
+    return all the stuff;
+  }
+});
+exports["some export"] = someImport
+return exports;
+})({}, __foo, __foo);
\ No newline at end of file

```