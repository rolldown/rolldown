# Diff
## /out.js
### esbuild
```js
// loose/index.js
var loose_default = class {
  constructor() {
    __publicField(this, "foo");
  }
};

// strict/index.js
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

//#region loose/index.js
class loose_index_default {
	foo;
}

//#endregion
//#region strict/index.js
class strict_index_default {
	foo;
}

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
@@ -1,11 +1,7 @@
-var loose_default = class {
-    constructor() {
-        __publicField(this, "foo");
-    }
-};
-var strict_default = class {
-    constructor() {
-        __publicField(this, "foo");
-    }
-};
-console.log(loose_default, strict_default);
+class loose_index_default {
+    foo;
+}
+class strict_index_default {
+    foo;
+}
+console.log(loose_index_default, strict_index_default);

```