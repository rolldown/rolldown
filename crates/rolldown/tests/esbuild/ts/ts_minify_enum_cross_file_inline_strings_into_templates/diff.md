# Reason
1. not support const enum inline
# Diff
## /out.js
### esbuild
```js
// entry.ts
console.log(`
					SameFile.STR = str 1
					SameFile.NUM = 123
					CrossFile.STR = str 2
					CrossFile.NUM = 321
				`);
```
### rolldown
```js

//#region cross-file.ts
let CrossFile = /* @__PURE__ */ function(CrossFile) {
	CrossFile["STR"] = "str 2";
	CrossFile[CrossFile["NUM"] = 321] = "NUM";
	return CrossFile;
}({});

//#endregion
//#region entry.ts
var SameFile = /* @__PURE__ */ function(SameFile) {
	SameFile["STR"] = "str 1";
	SameFile[SameFile["NUM"] = 123] = "NUM";
	return SameFile;
}(SameFile || {});
console.log(`
	SameFile.STR = ${SameFile.STR}
	SameFile.NUM = ${SameFile.NUM}
	CrossFile.STR = ${CrossFile.STR}
	CrossFile.NUM = ${CrossFile.NUM}
`);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,16 @@
+var CrossFile = (function (CrossFile) {
+    CrossFile["STR"] = "str 2";
+    CrossFile[CrossFile["NUM"] = 321] = "NUM";
+    return CrossFile;
+})({});
+var SameFile = (function (SameFile) {
+    SameFile["STR"] = "str 1";
+    SameFile[SameFile["NUM"] = 123] = "NUM";
+    return SameFile;
+})(SameFile || ({}));
 console.log(`
-					SameFile.STR = str 1
-					SameFile.NUM = 123
-					CrossFile.STR = str 2
-					CrossFile.NUM = 321
-				`);
+	SameFile.STR = ${SameFile.STR}
+	SameFile.NUM = ${SameFile.NUM}
+	CrossFile.STR = ${CrossFile.STR}
+	CrossFile.NUM = ${CrossFile.NUM}
+`);

```