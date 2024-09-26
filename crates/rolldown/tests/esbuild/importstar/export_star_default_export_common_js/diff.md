## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(entry_exports);

// foo.js
var foo = "foo";
```
### rolldown
```js

//#region foo.js
let foo = "foo";

//#endregion
export { foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,4 +1,4 @@
-var entry_exports = {};
-__export(entry_exports, { foo: () => foo });
-module.exports = __toCommonJS(entry_exports);
-var foo = 'foo';
\ No newline at end of file
+var foo = 'foo';
+export {
+    foo
+};
\ No newline at end of file

```
