# Diff
## /out/object.js
### esbuild
```js
// object.js
var keep1 = { *[Symbol.iterator]() {
}, [keep]: null };
var keep2 = { [keep]: null, *[Symbol.iterator]() {
} };
var keep3 = { *[Symbol.wtf]() {
} };
```
### rolldown
```js
//#region object.js
({
	*[Symbol.iterator]() {},
	[keep]: null
});
({
	[keep]: null,
	*[Symbol.iterator]() {}
});
({ *[Symbol.wtf]() {} });

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/object.js
+++ rolldown	object.js
@@ -1,11 +1,11 @@
-var keep1 = {
+({
     *[Symbol.iterator]() {},
     [keep]: null
-};
-var keep2 = {
+});
+({
     [keep]: null,
     *[Symbol.iterator]() {}
-};
-var keep3 = {
+});
+({
     *[Symbol.wtf]() {}
-};
+});

```