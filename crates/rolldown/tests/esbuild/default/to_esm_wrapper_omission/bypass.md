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

//#endregion

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
@@ -1,21 +1,27 @@
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