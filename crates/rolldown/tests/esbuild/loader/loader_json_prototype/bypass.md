# Reason
1. esbuild will inline named export if only default export is used.
2. could be done in minifier
# Diff
## /out.js
### esbuild
```js
// data.json
var data_default = {
  "": "The property below should be converted to a computed property:",
  ["__proto__"]: { foo: "bar" }
};

// entry.js
console.log(data_default);
```
### rolldown
```js

//#region data.json
var __proto__ = { "foo": "bar" };
var data_default = {
	"": "The property below should be converted to a computed property:",
	["__proto__"]: __proto__
};
//#endregion

//#region entry.js
console.log(data_default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,8 @@
+var __proto__ = {
+    "foo": "bar"
+};
 var data_default = {
     "": "The property below should be converted to a computed property:",
-    ["__proto__"]: {
-        foo: "bar"
-    }
+    ["__proto__"]: __proto__
 };
 console.log(data_default);

```