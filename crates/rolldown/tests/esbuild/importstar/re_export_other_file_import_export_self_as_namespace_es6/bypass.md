# Reason
1. rolldown generates module namespace object in the bottom if possible.
# Diff
## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  foo: () => foo,
  ns: () => foo_exports
});
var foo = 123;
export {
  foo,
  foo_exports as ns
};
```
### rolldown
```js


//#region foo.js
const foo = 123;
var foo_exports = {};
__export(foo_exports, {
	foo: () => foo,
	ns: () => foo_exports
});

//#endregion
export { foo, foo_exports as ns };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
+var foo = 123;
 var foo_exports = {};
 __export(foo_exports, {
     foo: () => foo,
     ns: () => foo_exports
 });
-var foo = 123;
 export {foo, foo_exports as ns};

```