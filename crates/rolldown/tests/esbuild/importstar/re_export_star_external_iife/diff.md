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
+++ rolldown	entry_js.mjs
@@ -1,5 +1,2 @@
-var mod = (() => {
-    var entry_exports = {};
-    __reExport(entry_exports, __require('foo'));
-    return __toCommonJS(entry_exports);
-})();
\ No newline at end of file
+(function () {
+}());
\ No newline at end of file

```
