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
console.log(entry_exports);
```
### rolldown
```js
"use strict";



//#region entry.js
var entry_exports = {};
__export(entry_exports, { foo: () => foo });
const foo = 123;
console.log(entry_exports);
//#endregion

exports.foo = foo
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
 var entry_exports = {};
 __export(entry_exports, {
     foo: () => foo
 });
-module.exports = __toCommonJS(entry_exports);
 var foo = 123;
 console.log(entry_exports);
+exports.foo = foo;

```