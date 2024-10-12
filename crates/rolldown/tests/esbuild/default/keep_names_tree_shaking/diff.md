# Diff
## /out.js
### esbuild
```js
// entry.js
function fnStmtKeep() {
}
__name(fnStmtKeep, "fnStmtKeep");
x = fnStmtKeep;
var fnExprKeep = /* @__PURE__ */ __name(function() {
}, "keep");
x = fnExprKeep;
var clsStmtKeep = class {
  static {
    __name(this, "clsStmtKeep");
  }
};
new clsStmtKeep();
var clsExprKeep = class {
  static {
    __name(this, "keep");
  }
};
new clsExprKeep();
```
### rolldown
```js

//#region entry.js
function fnStmtKeep() {}
x = fnStmtKeep;
let fnExprKeep = function keep() {};
x = fnExprKeep;
class clsStmtKeep {}
new clsStmtKeep();
let clsExprKeep = class keep {};
new clsExprKeep();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,17 +1,8 @@
 function fnStmtKeep() {}
-__name(fnStmtKeep, "fnStmtKeep");
 x = fnStmtKeep;
-var fnExprKeep = __name(function () {}, "keep");
+var fnExprKeep = function keep() {};
 x = fnExprKeep;
-var clsStmtKeep = class {
-    static {
-        __name(this, "clsStmtKeep");
-    }
-};
+class clsStmtKeep {}
 new clsStmtKeep();
-var clsExprKeep = class {
-    static {
-        __name(this, "keep");
-    }
-};
+var clsExprKeep = class keep {};
 new clsExprKeep();

```