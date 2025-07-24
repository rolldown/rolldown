# Reason
1. output should be same except the comment, acorn can not recognize using stmt
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
using null_keep = null;
await using await_null_keep = null;
using throw_keep = {};
using dispose_keep = { [Symbol.dispose]() {
  console.log("side effect");
} };
await using await_asyncDispose_keep = { [Symbol.asyncDispose]() {
  console.log("side effect");
} };
using undef_keep = void 0;
await using await_undef_keep = void 0;
console.log(
  null_keep,
  undef_keep
);
```
### rolldown
```js
//#region entry.js
using null_keep = null;
({ [Symbol.dispose]() {
	console.log("side effect");
} });
({ [Symbol.asyncDispose]() {
	console.log("side effect");
} });
using undef_keep = void 0;
console.log(null_keep, undef_keep);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,16 +1,13 @@
-using null_keep = null;
-await using await_null_keep = null;
-using throw_keep = {};
-using dispose_keep = {
+var null_keep = null;
+({
     [Symbol.dispose]() {
         console.log("side effect");
     }
-};
-await using await_asyncDispose_keep = {
+});
+({
     [Symbol.asyncDispose]() {
         console.log("side effect");
     }
-};
-using undef_keep = void 0;
-await using await_undef_keep = void 0;
+});
+var undef_keep = void 0;
 console.log(null_keep, undef_keep);

```