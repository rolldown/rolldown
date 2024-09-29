## /out.js
### esbuild
```js
var mod = (() => {
  var entry_exports = {};
  __reExport(entry_exports, require("foo"));
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
+++ rolldown	entry_js.mjs
@@ -1,5 +1,1 @@
-var mod = (() => {
-    var entry_exports = {};
-    __reExport(entry_exports, require("foo"));
-    return __toCommonJS(entry_exports);
-})();
+(function () {})();

```
