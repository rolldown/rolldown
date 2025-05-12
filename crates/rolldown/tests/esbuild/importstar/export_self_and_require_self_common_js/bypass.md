# Reason 
1.cjs module lexer can't recognize esbuild interop pattern
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
var foo;
var init_entry = __esm({
  "entry.js"() {
    foo = 123;
    console.log((init_entry(), __toCommonJS(entry_exports)));
  }
});
init_entry();
```
### rolldown
```js
"use strict";


//#region entry.js
var entry_exports = {};
__export(entry_exports, { foo: () => foo });
var foo;
var init_entry = __esm({ "entry.js"() {
	foo = 123;
	console.log((init_entry(), __toCommonJS(entry_exports)));
} });

//#endregion
init_entry();
exports.foo = foo;
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,13 +1,13 @@
 var entry_exports = {};
 __export(entry_exports, {
     foo: () => foo
 });
-module.exports = __toCommonJS(entry_exports);
 var foo;
 var init_entry = __esm({
     "entry.js"() {
         foo = 123;
         console.log((init_entry(), __toCommonJS(entry_exports)));
     }
 });
 init_entry();
+exports.foo = foo;

```