# Reason
1. rolldown generates module namespace object in the bottom if possible.
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
var foo = 123;
export {
  foo,
  entry_exports as ns
};
```
### rolldown
```js


//#region entry.js
const foo = 123;
var entry_exports = {};
__export(entry_exports, {
	foo: () => foo,
	ns: () => entry_exports
});

//#endregion
export { foo, entry_exports as ns };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
+var foo = 123;
 var entry_exports = {};
 __export(entry_exports, {
     foo: () => foo,
     ns: () => entry_exports
 });
-var foo = 123;
 export {foo, entry_exports as ns};

```