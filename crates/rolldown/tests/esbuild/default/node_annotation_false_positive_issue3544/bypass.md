# Reason
1. cjs module lexer can't recognize esbuild interop pattern
2. we have correct handle what esbuild did in https://github.com/evanw/esbuild/commit/109449e5b80886f7bc7fc7e0cee745a0221eef8d#diff-dce7544e65d7e31901bd98a9f0b88e6dfdc39a6e624d7e672e14224b2ec2d392R5281
3. the tiny difference is that esbuild only preserve `module` and `exports` if `format == 'cjs' &&  platform == 'node'`, here we follow rollup
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