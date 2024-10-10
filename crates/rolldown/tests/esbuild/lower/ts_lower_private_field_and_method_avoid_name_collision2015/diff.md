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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,16 +0,0 @@
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

```