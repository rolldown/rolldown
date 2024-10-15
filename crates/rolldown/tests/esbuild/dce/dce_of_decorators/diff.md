# Reason
1. lowering class
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
@fn class Class {}
class Field {
	@fn field;
}
class Method {
	@fn method() {}
}
class StaticField {
	@fn static field;
}
class StaticMethod {
	@fn static method() {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/keep-these.js
+++ rolldown	keep-these.js
@@ -1,28 +1,23 @@
-// decorator.js
-var fn = () => {
-  console.log("side effect");
-};
 
-// keep-these.js
-var Class = @fn class {
+//#region decorator.js
+const fn = () => {
+	console.log("side effect");
 };
-var Field = class {
-  @fn field;
-};
-var Method = class {
-  @fn method() {
-  }
-};
-var Accessor = class {
-  @fn accessor accessor;
-};
-var StaticField = class {
-  @fn static field;
-};
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
+//#region keep-these.js
+@fn class Class {}
+class Field {
+	@fn field;
+}
+class Method {
+	@fn method() {}
+}
+class StaticField {
+	@fn static field;
+}
+class StaticMethod {
+	@fn static method() {}
+}
+
+//#endregion
\ No newline at end of file

```