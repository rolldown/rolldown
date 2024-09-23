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
var entry_ns = {};
__export(entry_ns, {
	foo: () => foo,
	ns: () => entry_ns
});
const foo = 123;

//#endregion
Object.defineProperty(exports, 'foo', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
Object.defineProperty(exports, 'ns', {
  enumerable: true,
  get: function () {
    return entry_ns;
  }
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.cjs
@@ -1,7 +1,19 @@
-var entry_exports = {};
-__export(entry_exports, {
+'use strict';
+var entry_ns = {};
+__export(entry_ns, {
     foo: () => foo,
-    ns: () => entry_exports
+    ns: () => entry_ns
 });
-module.exports = __toCommonJS(entry_exports);
-var foo = 123;
\ No newline at end of file
+const foo = 123;
+Object.defineProperty(exports, 'foo', {
+    enumerable: true,
+    get: function () {
+        return foo;
+    }
+});
+Object.defineProperty(exports, 'ns', {
+    enumerable: true,
+    get: function () {
+        return entry_ns;
+    }
+});
\ No newline at end of file

```
