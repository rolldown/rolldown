# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  foo,
  init_a
} from "./chunk-PDZFCFBH.js";
init_a();
export {
  foo
};
```
### rolldown
```js
import { foo, init_a } from "./a2.js";

init_a();
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,3 +1,3 @@
-import {foo, init_a} from "./chunk-PDZFCFBH.js";
+import {foo, init_a} from "./a2.js";
 init_a();
 export {foo};

```
## /out/b.js
### esbuild
```js
import {
  __toCommonJS,
  a_exports,
  init_a
} from "./chunk-PDZFCFBH.js";

// b.js
var bar = (init_a(), __toCommonJS(a_exports));
export {
  bar
};
```
### rolldown
```js
import { __toCommonJS, a_exports, init_a } from "./a2.js";

//#region b.js
let bar = (init_a(), __toCommonJS(a_exports));

export { bar };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,3 +1,3 @@
-import {__toCommonJS, a_exports, init_a} from "./chunk-PDZFCFBH.js";
+import {__toCommonJS, a_exports, init_a} from "./a2.js";
 var bar = (init_a(), __toCommonJS(a_exports));
 export {bar};

```
## /out/chunk-PDZFCFBH.js
### esbuild
```js
// a.js
var a_exports = {};
__export(a_exports, {
  foo: () => foo
});
var foo;
var init_a = __esm({
  "a.js"() {
  }
});

export {
  __toCommonJS,
  foo,
  a_exports,
  init_a
};
```
### rolldown
```js

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
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

//#region a.js
var a_exports = {};
__export(a_exports, { foo: () => foo });
var foo;
var init_a = __esm({ "a.js"() {} });

export { __toCommonJS, a_exports, foo, init_a };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-PDZFCFBH.js
+++ rolldown	a2.js
@@ -1,17 +1,34 @@
-// a.js
+
+//#region rolldown:runtime
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
+var __esm = (fn, res) => function() {
+	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
+};
+var __export = (target, all) => {
+	for (var name in all) __defProp(target, name, {
+		get: all[name],
+		enumerable: true
+	});
+};
+var __copyProps = (to, from, except, desc) => {
+	if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
+		key = keys[i];
+		if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
+			get: ((k) => from[k]).bind(null, key),
+			enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
+		});
+	}
+	return to;
+};
+var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);
+
+//#region a.js
 var a_exports = {};
-__export(a_exports, {
-  foo: () => foo
-});
+__export(a_exports, { foo: () => foo });
 var foo;
-var init_a = __esm({
-  "a.js"() {
-  }
-});
+var init_a = __esm({ "a.js"() {} });
 
-export {
-  __toCommonJS,
-  foo,
-  a_exports,
-  init_a
-};
\ No newline at end of file
+export { __toCommonJS, a_exports, foo, init_a };
\ No newline at end of file

```