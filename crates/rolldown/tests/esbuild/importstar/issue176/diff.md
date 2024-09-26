## /out.js
### esbuild
```js
// folders/index.js
var folders_exports = {};
__export(folders_exports, {
  foo: () => foo
});

// folders/child/foo.js
var foo = () => "hi there";

// entry.js
console.log(JSON.stringify(folders_exports));
```
### rolldown
```js


//#region folders/child/foo.js
const foo = () => "hi there";

//#endregion
//#region folders/index.js
var folders_index_exports = {};
__export(folders_index_exports, { foo: () => foo });

//#endregion
//#region entry.js
console.log(JSON.stringify(folders_index_exports));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,4 +1,4 @@
-var folders_exports = {};
-__export(folders_exports, { foo: () => foo });
 var foo = () => 'hi there';
-console.log(JSON.stringify(folders_exports));
\ No newline at end of file
+var folders_index_exports = {};
+__export(folders_index_exports, { foo: () => foo });
+console.log(JSON.stringify(folders_index_exports));
\ No newline at end of file

```
