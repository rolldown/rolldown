# Reason
1. dce decorator
# Diff
## /out/keep-these.js
### esbuild
```js
// decorator.js
var fn = () => {
  console.log("side effect");
};

// keep-these.js
var Class = @fn class {
};
var Field = class {
  @fn field;
};
var Method = class {
  @fn method() {
  }
};
var Accessor = class {
  @fn accessor accessor;
};
var StaticField = class {
  @fn static field;
};
var StaticMethod = class {
  @fn static method() {
  }
};
var StaticAccessor = class {
  @fn static accessor accessor;
};
```
### rolldown
```js
//#region decorator.js
const fn = () => {
	console.log("side effect");
};

//#endregion
//#region keep-these.js
var Field = class {
	@fn field;
};
var StaticField = class {
	@fn static field;
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/keep-these.js
+++ rolldown	keep-these.js
@@ -1,28 +1,15 @@
-// decorator.js
-var fn = () => {
-  console.log("side effect");
+//#region decorator.js
+const fn = () => {
+	console.log("side effect");
 };
 
-// keep-these.js
-var Class = @fn class {
-};
+//#endregion
+//#region keep-these.js
 var Field = class {
-  @fn field;
+	@fn field;
 };
-var Method = class {
-  @fn method() {
-  }
-};
-var Accessor = class {
-  @fn accessor accessor;
-};
 var StaticField = class {
-  @fn static field;
+	@fn static field;
 };
-var StaticMethod = class {
-  @fn static method() {
-  }
-};
-var StaticAccessor = class {
-  @fn static accessor accessor;
-};
\ No newline at end of file
+
+//#endregion
\ No newline at end of file

```