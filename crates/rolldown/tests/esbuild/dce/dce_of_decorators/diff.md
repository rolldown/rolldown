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

```
### diff
```diff
===================================================================
--- esbuild	/out/keep-these.js
+++ rolldown	
@@ -1,28 +0,0 @@
-// decorator.js
-var fn = () => {
-  console.log("side effect");
-};
-
-// keep-these.js
-var Class = @fn class {
-};
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

```