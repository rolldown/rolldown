# Reason
1. redundant `import` statements
2. should not generate `__toCommonJS`
# Diff
## /out.js
### esbuild
```js
// a.js
var a_exports = {};
__export(a_exports, {
  ns: () => ns
});
import * as ns from "x";
var init_a = __esm({
  "a.js"() {
  }
});

// b.js
var b_exports = {};
__export(b_exports, {
  ns: () => ns2
});
import * as ns2 from "x";
var init_b = __esm({
  "b.js"() {
  }
});

// c.js
var c_exports = {};
__export(c_exports, {
  ns: () => ns3
});
import * as ns3 from "x";
var init_c = __esm({
  "c.js"() {
  }
});

// d.js
var d_exports = {};
__export(d_exports, {
  ns: () => ns4
});
import { ns as ns4 } from "x";
var init_d = __esm({
  "d.js"() {
  }
});

// e.js
var e_exports = {};
import * as x_star from "x";
var init_e = __esm({
  "e.js"() {
    __reExport(e_exports, x_star);
  }
});

// entry.js
init_a();
init_b();
init_c();
init_d();
init_e();
```
### rolldown
```js
import * as ns$3 from "x";
import * as ns$2 from "x";
import * as ns$1 from "x";
import { ns } from "x";

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __esm = (fn, res) => function() {
	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
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
var __reExport = (target, mod, secondTarget) => (__copyProps(target, mod, "default"), secondTarget && __copyProps(secondTarget, mod, "default"));

//#region a.js
var a_exports = {};
__export(a_exports, { ns: () => ns$3 });
var init_a = __esm({ "a.js"() {} });

//#region b.js
var b_exports = {};
__export(b_exports, { ns: () => ns$2 });
var init_b = __esm({ "b.js"() {} });

//#region c.js
var c_exports = {};
__export(c_exports, { ns: () => ns$1 });
var init_c = __esm({ "c.js"() {} });

//#region d.js
var d_exports = {};
__export(d_exports, { ns: () => ns });
var init_d = __esm({ "d.js"() {} });

//#region e.js
var e_exports = {};
import * as import_x from "x";
__reExport(e_exports, import_x);
var init_e = __esm({ "e.js"() {} });

//#region entry.js
init_a();
init_b();
init_c();
init_d();
init_e();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,42 +1,65 @@
+import * as ns$3 from "x";
+import * as ns$2 from "x";
+import * as ns$1 from "x";
+import {ns} from "x";
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
+var __esm = (fn, res) => function () {
+    return (fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res);
+};
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
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
+var __reExport = (target, mod, secondTarget) => (__copyProps(target, mod, "default"), secondTarget && __copyProps(secondTarget, mod, "default"));
 var a_exports = {};
 __export(a_exports, {
-    ns: () => ns
+    ns: () => ns$3
 });
-import * as ns from "x";
 var init_a = __esm({
     "a.js"() {}
 });
 var b_exports = {};
 __export(b_exports, {
-    ns: () => ns2
+    ns: () => ns$2
 });
-import * as ns2 from "x";
 var init_b = __esm({
     "b.js"() {}
 });
 var c_exports = {};
 __export(c_exports, {
-    ns: () => ns3
+    ns: () => ns$1
 });
-import * as ns3 from "x";
 var init_c = __esm({
     "c.js"() {}
 });
 var d_exports = {};
 __export(d_exports, {
-    ns: () => ns4
+    ns: () => ns
 });
-import {ns as ns4} from "x";
 var init_d = __esm({
     "d.js"() {}
 });
 var e_exports = {};
-import * as x_star from "x";
+import * as import_x from "x";
+__reExport(e_exports, import_x);
 var init_e = __esm({
-    "e.js"() {
-        __reExport(e_exports, x_star);
-    }
+    "e.js"() {}
 });
 init_a();
 init_b();
 init_c();

```