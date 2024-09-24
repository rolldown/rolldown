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
+++ rolldown	entry_js.cjs
@@ -1,4 +1,7 @@
-var entry_exports = {};
-__export(entry_exports, { out: () => out });
-module.exports = __toCommonJS(entry_exports);
-var out = __toESM(require('foo'));
\ No newline at end of file
+var out = __toESM(require('foo'));
+Object.defineProperty(exports, 'out', {
+    enumerable: true,
+    get: function () {
+        return out;
+    }
+});
\ No newline at end of file

```
