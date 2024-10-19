# Diff
## /out.js
### esbuild
```js
// loose/index.ts
var loose_default = class {
};

// strict/index.ts
var strict_default = class {
  constructor() {
    __publicField(this, "foo");
  }
};

// entry.js
console.log(loose_default, strict_default);
```
### rolldown
```js

//#region loose/index.ts
var loose_index_default = class {
	foo;
};

//#endregion
//#region strict/index.ts
var strict_index_default = class {
	foo;
};

//#endregion
//#region entry.js
console.log(loose_index_default, strict_index_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
-var loose_default = class {};
-var strict_default = class {
-    constructor() {
-        __publicField(this, "foo");
-    }
+var loose_index_default = class {
+    foo;
 };
-console.log(loose_default, strict_default);
+var strict_index_default = class {
+    foo;
+};
+console.log(loose_index_default, strict_index_default);

```