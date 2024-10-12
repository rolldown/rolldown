# Diff
## /out.js
### esbuild
```js
// entry.js
var A = class _A {
  static {
  }
  static {
    this.thisField++;
    _A.classField++;
    super.superField = super.superField + 1;
    super.superField++;
  }
};
var B = class {
  static {
  }
  static {
    this.thisField++;
    super.superField = super.superField + 1;
    super.superField++;
  }
};
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
@@ -1,13 +1,13 @@
-var A = class _A {
+class A {
     static {}
     static {
         this.thisField++;
-        _A.classField++;
+        A.classField++;
         super.superField = super.superField + 1;
         super.superField++;
     }
-};
+}
 var B = class {
     static {}
     static {
         this.thisField++;

```