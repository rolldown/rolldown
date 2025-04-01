# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  out: () => out
});
module.exports = __toCommonJS(entry_exports);
var out = __toESM(require("foo"));
```
### rolldown
```js
"use strict";


const foo = __toESM(require("foo"));

Object.defineProperty(exports, 'out', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,7 @@
-var entry_exports = {};
-__export(entry_exports, {
-    out: () => out
+var foo = __toESM(require("foo"));
+Object.defineProperty(exports, 'out', {
+    enumerable: true,
+    get: function () {
+        return foo;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var out = __toESM(require("foo"));

```