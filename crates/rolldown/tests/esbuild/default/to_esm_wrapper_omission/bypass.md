# Reason
1. codegen order
# Diff
## /out/entry.js
### esbuild
```js
var entry_exports = {};
module.exports = __toCommonJS(entry_exports);
var import_a_nowrap = require("a_nowrap");
var import_b_nowrap = require("b_nowrap");
__reExport(entry_exports, require("c_nowrap"), module.exports);
var d = __toESM(require("d_WRAP"));
var import_e_WRAP = __toESM(require("e_WRAP"));
var import_f_WRAP = __toESM(require("f_WRAP"));
var import_g_WRAP = __toESM(require("g_WRAP"));
var h = __toESM(require("h_WRAP"));
var i = __toESM(require("i_WRAP"));
var j = __toESM(require("j_WRAP"));
(0, import_b_nowrap.b)();
x = d.x;
(0, import_e_WRAP.default)();
(0, import_f_WRAP.default)();
(0, import_g_WRAP.__esModule)();
x = h;
i.x();
j.x``;
x = Promise.resolve().then(() => __toESM(require("k_WRAP")));
```
### rolldown
```js
"use strict";
//#region rolldown:runtime
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
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

require("a_nowrap");
const b_nowrap = __toESM(require("b_nowrap"));
const d_WRAP = __toESM(require("d_WRAP"));
const e_WRAP = __toESM(require("e_WRAP"));
const f_WRAP = __toESM(require("f_WRAP"));
const g_WRAP = __toESM(require("g_WRAP"));
const h_WRAP = __toESM(require("h_WRAP"));
const i_WRAP = __toESM(require("i_WRAP"));
const j_WRAP = __toESM(require("j_WRAP"));

//#region entry.js
(0, b_nowrap.b)();
x = d_WRAP.x;
(0, e_WRAP.default)();
(0, f_WRAP.default)();
(0, g_WRAP.__esModule)();
x = h_WRAP;
i_WRAP.x();
j_WRAP.x``;
x = import("k_WRAP");


var c_nowrap = require("c_nowrap");
Object.keys(c_nowrap).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return c_nowrap[k]; }
  });
});

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,21 +1,47 @@
-var entry_exports = {};
-module.exports = __toCommonJS(entry_exports);
-var import_a_nowrap = require("a_nowrap");
-var import_b_nowrap = require("b_nowrap");
-__reExport(entry_exports, require("c_nowrap"), module.exports);
-var d = __toESM(require("d_WRAP"));
-var import_e_WRAP = __toESM(require("e_WRAP"));
-var import_f_WRAP = __toESM(require("f_WRAP"));
-var import_g_WRAP = __toESM(require("g_WRAP"));
-var h = __toESM(require("h_WRAP"));
-var i = __toESM(require("i_WRAP"));
-var j = __toESM(require("j_WRAP"));
-(0, import_b_nowrap.b)();
-x = d.x;
-(0, import_e_WRAP.default)();
-(0, import_f_WRAP.default)();
-(0, import_g_WRAP.__esModule)();
-x = h;
-i.x();
-(j.x)``;
-x = Promise.resolve().then(() => __toESM(require("k_WRAP")));
+var __create = Object.create;
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __getProtoOf = Object.getPrototypeOf;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
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
+require("a_nowrap");
+var b_nowrap = __toESM(require("b_nowrap"));
+var d_WRAP = __toESM(require("d_WRAP"));
+var e_WRAP = __toESM(require("e_WRAP"));
+var f_WRAP = __toESM(require("f_WRAP"));
+var g_WRAP = __toESM(require("g_WRAP"));
+var h_WRAP = __toESM(require("h_WRAP"));
+var i_WRAP = __toESM(require("i_WRAP"));
+var j_WRAP = __toESM(require("j_WRAP"));
+(0, b_nowrap.b)();
+x = d_WRAP.x;
+(0, e_WRAP.default)();
+(0, f_WRAP.default)();
+(0, g_WRAP.__esModule)();
+x = h_WRAP;
+i_WRAP.x();
+(j_WRAP.x)``;
+x = import("k_WRAP");
+var c_nowrap = require("c_nowrap");
+Object.keys(c_nowrap).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return c_nowrap[k];
+        }
+    });
+});

```