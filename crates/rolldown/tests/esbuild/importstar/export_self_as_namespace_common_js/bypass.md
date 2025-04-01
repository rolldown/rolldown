# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  foo: () => foo,
  ns: () => entry_exports
});
module.exports = __toCommonJS(entry_exports);
var foo = 123;
```
### rolldown
```js
"use strict";



//#region entry.js
var entry_exports = {};
__export(entry_exports, {
	foo: () => foo,
	ns: () => entry_exports
});
const foo = 123;
//#endregion

exports.foo = foo
Object.defineProperty(exports, 'ns', {
  enumerable: true,
  get: function () {
    return entry_exports;
  }
});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -2,6 +2,12 @@
 __export(entry_exports, {
     foo: () => foo,
     ns: () => entry_exports
 });
-module.exports = __toCommonJS(entry_exports);
 var foo = 123;
+exports.foo = foo;
+Object.defineProperty(exports, 'ns', {
+    enumerable: true,
+    get: function () {
+        return entry_exports;
+    }
+});

```