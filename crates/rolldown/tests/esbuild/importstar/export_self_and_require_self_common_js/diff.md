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
var entry_exports, foo;
var init_entry = __esm({ "entry.js"() {
	entry_exports = {};
	__export(entry_exports, { foo: () => foo });
	foo = 123;
	console.log((init_entry(), __toCommonJS(entry_exports)));
} });

//#endregion
init_entry();
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
@@ -1,11 +1,16 @@
-var entry_exports = {};
-__export(entry_exports, { foo: () => foo });
-module.exports = __toCommonJS(entry_exports);
-var foo;
+var entry_exports, foo;
 var init_entry = __esm({
     'entry.js'() {
+        entry_exports = {};
+        __export(entry_exports, { foo: () => foo });
         foo = 123;
         console.log((init_entry(), __toCommonJS(entry_exports)));
     }
 });
-init_entry();
\ No newline at end of file
+init_entry();
+Object.defineProperty(exports, 'foo', {
+    enumerable: true,
+    get: function () {
+        return foo;
+    }
+});
\ No newline at end of file

```
