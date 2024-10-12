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
+++ rolldown	entry.js
@@ -1,6 +1,2 @@
-var entry_exports = {};
-__export(entry_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(entry_exports);
 var foo = "foo";
+export {foo};

```