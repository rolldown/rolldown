# Diff
## /out.js
### esbuild
```js
// entry.js
var _T_instances, a_fn, b_fn;
var T = class {
  constructor() {
    __privateAdd(this, _T_instances);
  }
  d() {
    console.log(__privateMethod(this, _T_instances, a_fn).call(this));
  }
};
_T_instances = new WeakSet();
a_fn = function() {
  return "a";
};
b_fn = function() {
  return "b";
};
__publicField(T, "c");
new T().d();
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region entry.js
class T {
	#a() {
		return "a";
	}
	#b() {
		return "b";
	}
	static c;
	d() {
		assert.equal(this.#a(), "a");
	}
}
new T().d();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,18 +1,13 @@
-var _T_instances, a_fn, b_fn;
-var T = class {
-    constructor() {
-        __privateAdd(this, _T_instances);
+class T {
+    #a() {
+        return "a";
     }
+    #b() {
+        return "b";
+    }
+    static c;
     d() {
-        console.log(__privateMethod(this, _T_instances, a_fn).call(this));
+        assert.equal(this.#a(), "a");
     }
-};
-_T_instances = new WeakSet();
-a_fn = function () {
-    return "a";
-};
-b_fn = function () {
-    return "b";
-};
-__publicField(T, "c");
+}
 new T().d();

```