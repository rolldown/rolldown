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
      /* @__PURE__ */ exports.factory("x", null),
      /* @__PURE__ */ exports.factory("x", null)
    ]);
    f = function() {
      console.log([
        /* @__PURE__ */ this.factory("y", null),
        /* @__PURE__ */ this.factory("y", null)
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
console.log([_jsx("x", {}), /* @__PURE__ */ this.factory("x", null)]);
f = function() {
	console.log([_jsx("y", {}), /* @__PURE__ */ this.factory("y", null)]);
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
-        console.log([exports.factory("x", null), exports.factory("x", null)]);
-        f = function () {
-            console.log([this.factory("y", null), this.factory("y", null)]);
-        };
-    }
-});
-export default require_factory();
+import {jsx as _jsx} from "react/jsx-runtime";
+console.log([_jsx("x", {}), this.factory("x", null)]);
+f = function () {
+    console.log([_jsx("y", {}), this.factory("y", null)]);
+};

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
var require_fragment = __commonJS({
  "fragment.jsx"(exports) {
    console.log([
      /* @__PURE__ */ exports.factory(exports.fragment, null, "x"),
      /* @__PURE__ */ exports.factory(exports.fragment, null, "x")
    ]), f = function() {
      console.log([
        /* @__PURE__ */ this.factory(this.fragment, null, "y"),
        /* @__PURE__ */ this.factory(this.fragment, null, "y")
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
console.log([_jsx(_Fragment, { children: "x" }), /* @__PURE__ */ this.factory(this.fragment, null, "x")]), f = function() {
	console.log([_jsx(_Fragment, { children: "y" }), /* @__PURE__ */ this.factory(this.fragment, null, "y")]);
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
-        (console.log([exports.factory(exports.fragment, null, "x"), exports.factory(exports.fragment, null, "x")]), f = function () {
-            console.log([this.factory(this.fragment, null, "y"), this.factory(this.fragment, null, "y")]);
-        });
-    }
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+(console.log([_jsx(_Fragment, {
+    children: "x"
+}), this.factory(this.fragment, null, "x")]), f = function () {
+    console.log([_jsx(_Fragment, {
+        children: "y"
+    }), this.factory(this.fragment, null, "y")]);
 });
-export default require_fragment();

```