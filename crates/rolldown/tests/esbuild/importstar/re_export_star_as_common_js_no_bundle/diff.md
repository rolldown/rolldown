# Diff
## /out.js
### esbuild
```js
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

const out = __toESM(require("foo"));

Object.defineProperty(exports, 'out', {
  enumerable: true,
  get: function () {
    return out;
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
-});
-module.exports = __toCommonJS(entry_exports);
 var out = __toESM(require("foo"));
+Object.defineProperty(exports, 'out', {
+    enumerable: true,
+    get: function () {
+        return out;
+    }
+});

```