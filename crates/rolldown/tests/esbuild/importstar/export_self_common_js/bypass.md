# Reason 
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
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

exports.foo = foo
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,2 @@
-var entry_exports = {};
-__export(entry_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(entry_exports);
 var foo = 123;
+exports.foo = foo;

```