# Diff
## /out.js
### esbuild
```js
// entry.js
var _A = class _A {
};
_A.thisField++;
_A.classField++;
__superSet(_A, _A, "superField", __superGet(_A, _A, "superField") + 1);
__superWrapper(_A, _A, "superField")._++;
var A = _A;
var _a;
var B = (_a = class {
}, _a.thisField++, __superSet(_a, _a, "superField", __superGet(_a, _a, "superField") + 1), __superWrapper(_a, _a, "superField")._++, _a);
```
### rolldown
```js

//#region entry.js
class A {
	static {}
	static {
		this.thisField++;
		A.classField++;
		super.superField = super.superField + 1;
		super.superField++;
	}
}
let B = class {
	static {}
	static {
		this.thisField++;
		super.superField = super.superField + 1;
		super.superField++;
	}
};

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,17 @@
-var _A = class _A {};
-_A.thisField++;
-_A.classField++;
-__superSet(_A, _A, "superField", __superGet(_A, _A, "superField") + 1);
-__superWrapper(_A, _A, "superField")._++;
-var A = _A;
-var _a;
-var B = (_a = class {}, _a.thisField++, __superSet(_a, _a, "superField", __superGet(_a, _a, "superField") + 1), __superWrapper(_a, _a, "superField")._++, _a);
+class A {
+    static {}
+    static {
+        this.thisField++;
+        A.classField++;
+        super.superField = super.superField + 1;
+        super.superField++;
+    }
+}
+var B = class {
+    static {}
+    static {
+        this.thisField++;
+        super.superField = super.superField + 1;
+        super.superField++;
+    }
+};

```