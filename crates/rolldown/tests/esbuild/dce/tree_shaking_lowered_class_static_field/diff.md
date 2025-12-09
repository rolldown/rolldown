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
// HIDDEN [\0@oxc-project+runtime@0.0.0/file.js]
// HIDDEN [\0@oxc-project+runtime@0.0.0/file.js]
// HIDDEN [\0@oxc-project+runtime@0.0.0/file.js]
// HIDDEN [\0@oxc-project+runtime@0.0.0/file.js]
//#region entry.js
var REMOVE_ME = class {};
_defineProperty(REMOVE_ME, "x", "x");
_defineProperty(REMOVE_ME, "y", "y");
_defineProperty(REMOVE_ME, "z", "z");
var KeepMe1 = class {};
_defineProperty(KeepMe1, "x", "x");
_defineProperty(KeepMe1, "y", sideEffects());
_defineProperty(KeepMe1, "z", "z");
var KeepMe2 = class {};
_defineProperty(KeepMe2, "x", "x");
_defineProperty(KeepMe2, "y", "y");
_defineProperty(KeepMe2, "z", "z");
new KeepMe2();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,9 +1,13 @@
+var REMOVE_ME = class {};
+_defineProperty(REMOVE_ME, "x", "x");
+_defineProperty(REMOVE_ME, "y", "y");
+_defineProperty(REMOVE_ME, "z", "z");
 var KeepMe1 = class {};
-__publicField(KeepMe1, "x", "x");
-__publicField(KeepMe1, "y", sideEffects());
-__publicField(KeepMe1, "z", "z");
+_defineProperty(KeepMe1, "x", "x");
+_defineProperty(KeepMe1, "y", sideEffects());
+_defineProperty(KeepMe1, "z", "z");
 var KeepMe2 = class {};
-__publicField(KeepMe2, "x", "x");
-__publicField(KeepMe2, "y", "y");
-__publicField(KeepMe2, "z", "z");
+_defineProperty(KeepMe2, "x", "x");
+_defineProperty(KeepMe2, "y", "y");
+_defineProperty(KeepMe2, "z", "z");
 new KeepMe2();

```