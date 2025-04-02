# Reason
1. not support const enum inline
# Diff
## /out/entry.js
### esbuild
```js
// enums.ts
var a = ((a2) => {
  a2[a2["implicit_number"] = 0] = "implicit_number";
  a2[a2["explicit_number"] = 123] = "explicit_number";
  a2["explicit_string"] = "xyz";
  a2[a2["non_constant"] = foo] = "non_constant";
  return a2;
})(a || {});

// entry.ts
console.log([
  0 /* implicit_number */,
  123 /* explicit_number */,
  "xyz" /* explicit_string */,
  a.non_constant
]);
```
### rolldown
```js

//#region enums.ts
let a = /* @__PURE__ */ function(a) {
	a[a["implicit_number"] = 0] = "implicit_number";
	a[a["explicit_number"] = 123] = "explicit_number";
	a["explicit_string"] = "xyz";
	a[a["non_constant"] = foo] = "non_constant";
	return a;
}({});

//#endregion
//#region entry.ts
console.log([
	a.implicit_number,
	a.explicit_number,
	a.explicit_string,
	a.non_constant
]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
-var a = (a2 => {
-    a2[a2["implicit_number"] = 0] = "implicit_number";
-    a2[a2["explicit_number"] = 123] = "explicit_number";
-    a2["explicit_string"] = "xyz";
-    a2[a2["non_constant"] = foo] = "non_constant";
-    return a2;
-})(a || ({}));
-console.log([0, 123, "xyz", a.non_constant]);
+var a = (function (a) {
+    a[a["implicit_number"] = 0] = "implicit_number";
+    a[a["explicit_number"] = 123] = "explicit_number";
+    a["explicit_string"] = "xyz";
+    a[a["non_constant"] = foo] = "non_constant";
+    return a;
+})({});
+console.log([a.implicit_number, a.explicit_number, a.explicit_string, a.non_constant]);

```