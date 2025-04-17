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
//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};


//#region entry.js
var entry_exports = {};
__export(entry_exports, {
	foo: () => foo,
	ns: () => entry_exports
});
const foo = 123;

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
@@ -1,7 +1,20 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
 var entry_exports = {};
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