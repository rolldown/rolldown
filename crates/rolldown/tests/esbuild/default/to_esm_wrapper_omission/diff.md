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
var c_nowrap = require("c_nowrap");
Object.keys(c_nowrap).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return c_nowrap[k]; }
  });
});

require("a_nowrap");
const { b } = __toESM(require("b_nowrap"));
require("c_nowrap");
const d = __toESM(require("d_WRAP"));
const { default: e } = __toESM(require("e_WRAP"));
const { default: f } = __toESM(require("f_WRAP"));
const { __esModule: g } = __toESM(require("g_WRAP"));
const h = __toESM(require("h_WRAP"));
const i = __toESM(require("i_WRAP"));
const j = __toESM(require("j_WRAP"));

//#region entry.js
b();
x = d.x;
e();
f();
g();
x = h;
i.x();
j.x` + "``" + `;
x = import("k_WRAP");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,21 +1,28 @@
-var entry_exports = {};
-module.exports = __toCommonJS(entry_exports);
-var import_a_nowrap = require("a_nowrap");
-var import_b_nowrap = require("b_nowrap");
-__reExport(entry_exports, require("c_nowrap"), module.exports);
+var c_nowrap = require("c_nowrap");
+Object.keys(c_nowrap).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return c_nowrap[k];
+        }
+    });
+});
+require("a_nowrap");
+var {b} = __toESM(require("b_nowrap"));
+require("c_nowrap");
 var d = __toESM(require("d_WRAP"));
-var import_e_WRAP = __toESM(require("e_WRAP"));
-var import_f_WRAP = __toESM(require("f_WRAP"));
-var import_g_WRAP = __toESM(require("g_WRAP"));
+var {default: e} = __toESM(require("e_WRAP"));
+var {default: f} = __toESM(require("f_WRAP"));
+var {__esModule: g} = __toESM(require("g_WRAP"));
 var h = __toESM(require("h_WRAP"));
 var i = __toESM(require("i_WRAP"));
 var j = __toESM(require("j_WRAP"));
-(0, import_b_nowrap.b)();
+b();
 x = d.x;
-(0, import_e_WRAP.default)();
-(0, import_f_WRAP.default)();
-(0, import_g_WRAP.__esModule)();
+e();
+f();
+g();
 x = h;
 i.x();
-(j.x)``;
-x = Promise.resolve().then(() => __toESM(require("k_WRAP")));
+(j.x)` + "``" + `;
+x = import("k_WRAP");

```