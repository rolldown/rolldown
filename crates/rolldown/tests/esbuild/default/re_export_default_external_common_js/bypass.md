# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  bar: () => import_bar.default,
  foo: () => import_foo.default
});
module.exports = __toCommonJS(entry_exports);
var import_foo = __toESM(require("foo"));

// bar.js
var import_bar = __toESM(require("bar"));
```
### rolldown
```js
"use strict";


const foo = __toESM(require("foo"));
const bar = __toESM(require("bar"));

Object.defineProperty(exports, 'bar', {
  enumerable: true,
  get: function () {
    return bar.default;
  }
});
Object.defineProperty(exports, 'foo', {
  enumerable: true,
  get: function () {
    return foo.default;
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
+var foo = __toESM(require("foo"));
+var bar = __toESM(require("bar"));
+Object.defineProperty(exports, 'bar', {
+    enumerable: true,
+    get: function () {
+        return bar.default;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = __toESM(require("foo"));
-var import_bar = __toESM(require("bar"));
+Object.defineProperty(exports, 'foo', {
+    enumerable: true,
+    get: function () {
+        return foo.default;
+    }
+});

```