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
let CrossFile = /* @__PURE__ */ function(CrossFile$1) {
	CrossFile$1["STR"] = "str 2";
	CrossFile$1[CrossFile$1["NUM"] = 321] = "NUM";
	return CrossFile$1;
}({});

//#endregion
//#region entry.ts
var SameFile = /* @__PURE__ */ function(SameFile$1) {
	SameFile$1["STR"] = "str 1";
	SameFile$1[SameFile$1["NUM"] = 123] = "NUM";
	return SameFile$1;
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
+var CrossFile = (function (CrossFile$1) {
+    CrossFile$1["STR"] = "str 2";
+    CrossFile$1[CrossFile$1["NUM"] = 321] = "NUM";
+    return CrossFile$1;
+})({});
+var SameFile = (function (SameFile$1) {
+    SameFile$1["STR"] = "str 1";
+    SameFile$1[SameFile$1["NUM"] = 123] = "NUM";
+    return SameFile$1;
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