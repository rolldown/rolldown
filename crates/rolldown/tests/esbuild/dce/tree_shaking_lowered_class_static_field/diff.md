# Reason
1. lowering class
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var KeepMe1 = class {
};
__publicField(KeepMe1, "x", "x");
__publicField(KeepMe1, "y", sideEffects());
__publicField(KeepMe1, "z", "z");
var KeepMe2 = class {
};
__publicField(KeepMe2, "x", "x");
__publicField(KeepMe2, "y", "y");
__publicField(KeepMe2, "z", "z");
new KeepMe2();
```
### rolldown
```js
//#region entry.js
var KeepMe1 = class {
	static x = "x";
	static y = sideEffects();
	static z = "z";
};
var KeepMe2 = class {
	static x = "x";
	static y = "y";
	static z = "z";
};
new KeepMe2();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,9 +1,11 @@
-var KeepMe1 = class {};
-__publicField(KeepMe1, "x", "x");
-__publicField(KeepMe1, "y", sideEffects());
-__publicField(KeepMe1, "z", "z");
-var KeepMe2 = class {};
-__publicField(KeepMe2, "x", "x");
-__publicField(KeepMe2, "y", "y");
-__publicField(KeepMe2, "z", "z");
+var KeepMe1 = class {
+    static x = "x";
+    static y = sideEffects();
+    static z = "z";
+};
+var KeepMe2 = class {
+    static x = "x";
+    static y = "y";
+    static z = "z";
+};
 new KeepMe2();

```