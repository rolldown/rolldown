# Reason
1. not align
# Diff
## /out.js
### esbuild
```js
// entry.mjs
var entry_exports = {};
__export(entry_exports, {
  confuseNode: () => confuseNode
});
module.exports = __toCommonJS(entry_exports);
function confuseNode(exports2) {
  exports2.notAnExport = function() {
  };
}
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  confuseNode
});
```
### rolldown
```js
"use strict";

//#region entry.mjs
function confuseNode(exports$1) {
	exports$1.notAnExport = function() {};
}

//#endregion
exports.confuseNode = confuseNode
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,4 @@
-var entry_exports = {};
-__export(entry_exports, {
-    confuseNode: () => confuseNode
-});
-module.exports = __toCommonJS(entry_exports);
-function confuseNode(exports2) {
-    exports2.notAnExport = function () {};
+function confuseNode(exports$1) {
+    exports$1.notAnExport = function () {};
 }
-0 && (module.exports = {
-    confuseNode
-});
+exports.confuseNode = confuseNode;

```