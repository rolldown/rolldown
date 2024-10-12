# Diff
## /out.js
### esbuild
```js
// b.js
var Base = class {
};

// a.js
var Keep = class extends Base {
};

// entry.js
new Keep();
```
### rolldown
```js

//#region b.js
class Base {}

//#endregion
//#region a.js
class Keep extends Base {}

//#endregion
//#region entry.js
new Keep();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-var Base = class {};
-var Keep = class extends Base {};
+class Base {}
+class Keep extends Base {}
 new Keep();

```