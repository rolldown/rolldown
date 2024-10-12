# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var KeepMe1 = class {
};
__publicField(KeepMe1, "x", "x"), __publicField(KeepMe1, "y", sideEffects()), __publicField(KeepMe1, "z", "z");
var KeepMe2 = class {
};
__publicField(KeepMe2, "x", "x"), __publicField(KeepMe2, "y", "y"), __publicField(KeepMe2, "z", "z");
new KeepMe2();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var KeepMe1 = class {};
-(__publicField(KeepMe1, "x", "x"), __publicField(KeepMe1, "y", sideEffects()), __publicField(KeepMe1, "z", "z"));
-var KeepMe2 = class {};
-(__publicField(KeepMe2, "x", "x"), __publicField(KeepMe2, "y", "y"), __publicField(KeepMe2, "z", "z"));
-new KeepMe2();

```