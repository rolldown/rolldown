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
@@ -1,7 +1,5 @@
 var data_default = {
     "": "The property below should be converted to a computed property:",
-    ["__proto__"]: {
-        foo: "bar"
-    }
+    ["__proto__"]: __proto__
 };
 console.log(data_default);

```