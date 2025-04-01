# Diff
## /out.js
### esbuild
```js
// entry.js
function foo(RegExp2) {
  return new RegExp2(new RegExp(".", "d"), "d");
}
export {
  foo
};
```
### rolldown
```js

//#region entry.js
function foo(RegExp) {
	return new RegExp(/./d, "d");
}
//#endregion

export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-function foo(RegExp2) {
-    return new RegExp2(new RegExp(".", "d"), "d");
+function foo(RegExp) {
+    return new RegExp(/./d, "d");
 }
 export {foo};

```