## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(entry_exports);
var foo = 123;
```
### rolldown
```js
"use strict";

//#region entry.js
const foo = 123;

//#endregion
Object.defineProperty(exports, 'foo', {
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
+++ rolldown	entry_js.cjs
@@ -1,4 +1,8 @@
-var entry_exports = {};
-__export(entry_exports, { foo: () => foo });
-module.exports = __toCommonJS(entry_exports);
-var foo = 123;
\ No newline at end of file
+'use strict';
+const foo = 123;
+Object.defineProperty(exports, 'foo', {
+    enumerable: true,
+    get: function () {
+        return foo;
+    }
+});
\ No newline at end of file

```
