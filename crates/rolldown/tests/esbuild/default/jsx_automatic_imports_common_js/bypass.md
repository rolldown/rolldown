# Reason
1. different naming style for `oxc_transformer`
# Diff
## /out.js
### esbuild
```js
// custom-react.js
var require_custom_react = __commonJS({
  "custom-react.js"(exports, module) {
    module.exports = {};
  }
});

// entry.jsx
var import_custom_react = __toESM(require_custom_react());
import { Fragment as Fragment2, jsx as jsx2 } from "react/jsx-runtime";
console.log(/* @__PURE__ */ jsx2("div", { jsx: import_custom_react.jsx }), /* @__PURE__ */ jsx2(Fragment2, { children: /* @__PURE__ */ jsx2(import_custom_react.Fragment, {}) }));
```
### rolldown
```js
import { Fragment, jsx } from "react/jsx-runtime";

//#region rolldown:runtime
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};
var __copyProps = (to, from, except, desc) => {
	if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
		key = keys[i];
		if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
			get: ((k) => from[k]).bind(null, key),
			enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
		});
	}
	return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
	value: mod,
	enumerable: true
}) : target, mod));

//#region custom-react.js
var require_custom_react = __commonJS({ "custom-react.js"(exports, module) {
	module.exports = {};
} });
var import_custom_react = __toESM(require_custom_react());

//#region entry.jsx
console.log(/* @__PURE__ */ jsx("div", { jsx: import_custom_react.jsx }), /* @__PURE__ */ jsx(Fragment, { children: /* @__PURE__ */ jsx(import_custom_react.Fragment, {}) }));

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,37 @@
+import {Fragment, jsx} from "react/jsx-runtime";
+var __create = Object.create;
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __getProtoOf = Object.getPrototypeOf;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var __copyProps = (to, from, except, desc) => {
+    if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
+        key = keys[i];
+        if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
+            get: (k => from[k]).bind(null, key),
+            enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
+        });
+    }
+    return to;
+};
+var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
+    value: mod,
+    enumerable: true
+}) : target, mod));
 var require_custom_react = __commonJS({
     "custom-react.js"(exports, module) {
         module.exports = {};
     }
 });
 var import_custom_react = __toESM(require_custom_react());
-import {Fragment as Fragment2, jsx as jsx2} from "react/jsx-runtime";
-console.log(jsx2("div", {
+console.log(jsx("div", {
     jsx: import_custom_react.jsx
-}), jsx2(Fragment2, {
-    children: jsx2(import_custom_react.Fragment, {})
+}), jsx(Fragment, {
+    children: jsx(import_custom_react.Fragment, {})
 }));

```