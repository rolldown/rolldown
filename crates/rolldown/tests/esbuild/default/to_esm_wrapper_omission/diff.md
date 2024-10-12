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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,21 +0,0 @@
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

```