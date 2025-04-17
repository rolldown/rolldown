# Reason
1. Can't disable bundle splitting
# Diff
## /out.js
### esbuild
```js
// c.js
var c_exports = {};
var init_c = __esm({
  async "c.js"() {
    await 0;
  }
});

// b.js
var b_exports = {};
var init_b = __esm({
  async "b.js"() {
    await init_c();
  }
});

// a.js
var a_exports = {};
var init_a = __esm({
  async "a.js"() {
    await init_b();
  }
});

// entry.js
var entry_exports = {};
var init_entry = __esm({
  async "entry.js"() {
    init_a();
    init_b();
    init_c();
    init_entry();
    await 0;
  }
});
await init_entry();
```
### rolldown
```js

//#region rolldown:runtime
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __esm = (fn, res) => function() {
	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
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

//#region entry.js
var require_entry = __commonJS({ "entry.js"() {
	Promise.resolve().then(function() {
		return init_a(), a_exports;
	});
	Promise.resolve().then(function() {
		return init_b(), b_exports;
	});
	Promise.resolve().then(function() {
		return __toESM(require_c());
	});
	Promise.resolve().then(function() {
		return __toESM(require_entry());
	});
	await 0;
} });

//#region a.js
var a_exports = {};
var init_a = __esm({ async "a.js"() {
	await init_b();
} });

//#region b.js
var b_exports = {};
var init_b = __esm({ async "b.js"() {} });

//#region c.js
var require_c = __commonJS({ "c.js"() {
	await 0;
} });

export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,62 @@
-var c_exports = {};
-var init_c = __esm({
-    async "c.js"() {
-        await 0;
-    }
-});
-var b_exports = {};
-var init_b = __esm({
-    async "b.js"() {
-        await init_c();
-    }
-});
+
+//#region rolldown:runtime
+var __create = Object.create;
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __getProtoOf = Object.getPrototypeOf;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
+var __esm = (fn, res) => function() {
+	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
+};
+var __commonJS = (cb, mod) => function() {
+	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
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
+var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
+	value: mod,
+	enumerable: true
+}) : target, mod));
+
+//#region entry.js
+var require_entry = __commonJS({ "entry.js"() {
+	Promise.resolve().then(function() {
+		return init_a(), a_exports;
+	});
+	Promise.resolve().then(function() {
+		return init_b(), b_exports;
+	});
+	Promise.resolve().then(function() {
+		return __toESM(require_c());
+	});
+	Promise.resolve().then(function() {
+		return __toESM(require_entry());
+	});
+	await 0;
+} });
+
+//#region a.js
 var a_exports = {};
-var init_a = __esm({
-    async "a.js"() {
-        await init_b();
-    }
-});
-var entry_exports = {};
-var init_entry = __esm({
-    async "entry.js"() {
-        init_a();
-        init_b();
-        init_c();
-        init_entry();
-        await 0;
-    }
-});
-await init_entry();
+var init_a = __esm({ async "a.js"() {
+	await init_b();
+} });
+
+//#region b.js
+var b_exports = {};
+var init_b = __esm({ async "b.js"() {} });
+
+//#region c.js
+var require_c = __commonJS({ "c.js"() {
+	await 0;
+} });
+
+export default require_entry();

```