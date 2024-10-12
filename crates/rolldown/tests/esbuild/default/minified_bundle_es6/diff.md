# Diff
## /out.js
### esbuild
```js
function o(){return 123}o();console.log(o());
```
### rolldown
```js

//#region a.js
function foo() {
	return 123;
}
foo();

//#endregion
//#region entry.js
console.log(foo());

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-function o() {
+function foo() {
     return 123;
 }
-o();
-console.log(o());
+foo();
+console.log(foo());

```