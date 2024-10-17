# Reason
1. `jsx.factory`
# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
var require_factory = __commonJS({
  "factory.jsx"(exports) {
    console.log([
      /* @__PURE__ */ exports("x", null),
      /* @__PURE__ */ exports("x", null)
    ]);
    f = function() {
      console.log([
        /* @__PURE__ */ this("y", null),
        /* @__PURE__ */ this("y", null)
      ]);
    };
  }
});
export default require_factory();
```
### rolldown
```js
import { jsx as _jsx } from "react/jsx-runtime";

//#region factory.jsx
console.log([_jsx("x", {}), /* @__PURE__ */ this("x", null)]);
f = function() {
	console.log([_jsx("y", {}), /* @__PURE__ */ this("y", null)]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,9 +1,5 @@
-var require_factory = __commonJS({
-    "factory.jsx"(exports) {
-        console.log([exports("x", null), exports("x", null)]);
-        f = function () {
-            console.log([this("y", null), this("y", null)]);
-        };
-    }
-});
-export default require_factory();
+import {jsx as _jsx} from "react/jsx-runtime";
+console.log([_jsx("x", {}), this("x", null)]);
+f = function () {
+    console.log([_jsx("y", {}), this("y", null)]);
+};

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
var require_fragment = __commonJS({
  "fragment.jsx"(exports) {
    console.log([
      /* @__PURE__ */ exports(exports, null, "x"),
      /* @__PURE__ */ exports(exports, null, "x")
    ]), f = function() {
      console.log([
        /* @__PURE__ */ this(this, null, "y"),
        /* @__PURE__ */ this(this, null, "y")
      ]);
    };
  }
});
export default require_fragment();
```
### rolldown
```js
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region fragment.jsx
console.log([_jsx(_Fragment, { children: "x" }), /* @__PURE__ */ this(this, null, "x")]), f = function() {
	console.log([_jsx(_Fragment, { children: "y" }), /* @__PURE__ */ this(this, null, "y")]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,8 +1,8 @@
-var require_fragment = __commonJS({
-    "fragment.jsx"(exports) {
-        (console.log([exports(exports, null, "x"), exports(exports, null, "x")]), f = function () {
-            console.log([this(this, null, "y"), this(this, null, "y")]);
-        });
-    }
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+(console.log([_jsx(_Fragment, {
+    children: "x"
+}), this(this, null, "x")]), f = function () {
+    console.log([_jsx(_Fragment, {
+        children: "y"
+    }), this(this, null, "y")]);
 });
-export default require_fragment();

```