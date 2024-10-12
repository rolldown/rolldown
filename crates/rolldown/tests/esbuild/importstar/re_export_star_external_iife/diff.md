# Diff
## /out.js
### esbuild
```js
var mod = (() => {
  // entry.js
  var entry_exports = {};
  __reExport(entry_exports, __require("foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
(function() {

"use strict";

})();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,1 @@
-var mod = (() => {
-    var entry_exports = {};
-    __reExport(entry_exports, __require("foo"));
-    return __toCommonJS(entry_exports);
-})();
+(function () {})();

```