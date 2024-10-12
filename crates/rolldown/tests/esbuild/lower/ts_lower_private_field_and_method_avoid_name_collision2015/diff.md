# Diff
## /out.js
### esbuild
```js
// entry.ts
var _x;
var WeakMap2 = class {
  constructor() {
    __privateAdd(this, _x);
  }
};
_x = new WeakMap();
var _WeakSet_instances, y_fn;
var WeakSet2 = class {
  constructor() {
    __privateAdd(this, _WeakSet_instances);
  }
};
_WeakSet_instances = new WeakSet();
y_fn = function() {
};
export {
  WeakMap2 as WeakMap,
  WeakSet2 as WeakSet
};
```
### rolldown
```js

//#region entry.ts
class WeakMap {
	#x;
}
class WeakSet {
	#y() {}
}

//#endregion
export { WeakMap, WeakSet };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,16 +1,7 @@
-var _x;
-var WeakMap2 = class {
-    constructor() {
-        __privateAdd(this, _x);
-    }
-};
-_x = new WeakMap();
-var _WeakSet_instances, y_fn;
-var WeakSet2 = class {
-    constructor() {
-        __privateAdd(this, _WeakSet_instances);
-    }
-};
-_WeakSet_instances = new WeakSet();
-y_fn = function () {};
-export {WeakMap2 as WeakMap, WeakSet2 as WeakSet};
+class WeakMap {
+    #x;
+}
+class WeakSet {
+    #y() {}
+}
+export {WeakMap, WeakSet};

```