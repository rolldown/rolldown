# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
var entry_exports = {};
__export(entry_exports, {
  bar: () => import_bar.default,
  foo: () => import_foo.default
});
module.exports = __toCommonJS(entry_exports);
var import_foo = __toESM(require("./foo"));
var import_bar = __toESM(require("./bar"));
```
### rolldown
```js
"use strict";

const ___foo = __toESM(require("./foo"));
const ___bar = __toESM(require("./bar"));

Object.defineProperty(exports, 'bar', {
  enumerable: true,
  get: function () {
    return ___bar.default;
  }
});
Object.defineProperty(exports, 'foo', {
  enumerable: true,
  get: function () {
    return ___foo.default;
  }
});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,14 @@
-var entry_exports = {};
-__export(entry_exports, {
-    bar: () => import_bar.default,
-    foo: () => import_foo.default
+var ___foo = __toESM(require("./foo"));
+var ___bar = __toESM(require("./bar"));
+Object.defineProperty(exports, 'bar', {
+    enumerable: true,
+    get: function () {
+        return ___bar.default;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = __toESM(require("./foo"));
-var import_bar = __toESM(require("./bar"));
+Object.defineProperty(exports, 'foo', {
+    enumerable: true,
+    get: function () {
+        return ___foo.default;
+    }
+});

```