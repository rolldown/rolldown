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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var entry_exports = {};
-__export(entry_exports, {
-    confuseNode: () => confuseNode
-});
-module.exports = __toCommonJS(entry_exports);
-function confuseNode(exports2) {
-    exports2.notAnExport = function () {};
-}
-0 && (module.exports = {
-    confuseNode
-});

```