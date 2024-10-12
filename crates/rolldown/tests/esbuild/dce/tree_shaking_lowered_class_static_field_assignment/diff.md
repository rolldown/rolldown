# Diff
## /out/entry.js
### esbuild
```js
// entry.ts
var KeepMe1 = class {
};
KeepMe1.x = "x";
KeepMe1.y = "y";
KeepMe1.z = "z";
var KeepMe2 = class {
};
KeepMe2.x = "x";
KeepMe2.y = sideEffects();
KeepMe2.z = "z";
var KeepMe3 = class {
};
KeepMe3.x = "x";
KeepMe3.y = "y";
KeepMe3.z = "z";
new KeepMe3();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var KeepMe1 = class {};
-KeepMe1.x = "x";
-KeepMe1.y = "y";
-KeepMe1.z = "z";
-var KeepMe2 = class {};
-KeepMe2.x = "x";
-KeepMe2.y = sideEffects();
-KeepMe2.z = "z";
-var KeepMe3 = class {};
-KeepMe3.x = "x";
-KeepMe3.y = "y";
-KeepMe3.z = "z";
-new KeepMe3();

```