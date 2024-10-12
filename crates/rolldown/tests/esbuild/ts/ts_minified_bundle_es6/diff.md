# Diff
## /out.js
### esbuild
```js
function o(){return 123}console.log(o());
```
### rolldown
```js

//#region a.ts
function foo() {
	return 123;
}

//#endregion
//#region entry.ts
console.log(foo());

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-function o() {
+function foo() {
     return 123;
 }
-console.log(o());
+console.log(foo());

```