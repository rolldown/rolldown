# Reason
1. lowering class
# Diff
## /out/class.js
### esbuild
```js
// class.js
var Keep1 = class {
  *[Symbol.iterator]() {
  }
  [keep];
};
var Keep2 = class {
  [keep];
  *[Symbol.iterator]() {
  }
};
var Keep3 = class {
  *[Symbol.wtf]() {
  }
};
```
### rolldown
```js

//#region class.js
class Keep1 {
	*[Symbol.iterator]() {}
	[keep];
}
class Keep2 {
	[keep];
	*[Symbol.iterator]() {}
}
class Keep3 {
	*[Symbol.wtf]() {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/class.js
+++ rolldown	class.js
@@ -1,11 +1,11 @@
-var Keep1 = class {
+class Keep1 {
     *[Symbol.iterator]() {}
     [keep];
-};
-var Keep2 = class {
+}
+class Keep2 {
     [keep];
     *[Symbol.iterator]() {}
-};
-var Keep3 = class {
+}
+class Keep3 {
     *[Symbol.wtf]() {}
-};
+}

```