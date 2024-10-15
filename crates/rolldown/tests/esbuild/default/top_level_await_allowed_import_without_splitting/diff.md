# Reason
1. not align
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

//#endregion
//#region a.js
var a_exports;
var init_a = __esm({ "a.js"() {
	a_exports = {};
	init_b();
} });

//#endregion
//#region b.js
var b_exports, import_c;
var init_b = __esm({ "b.js"() {
	b_exports = {};
	import_c = __toESM(require_c());
} });

//#endregion
//#region c.js
var require_c = __commonJS({ "c.js"() {
	await 0;
} });

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,43 @@
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
-var a_exports = {};
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
+
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
+//#endregion
+//#region a.js
+var a_exports;
+var init_a = __esm({ "a.js"() {
+	a_exports = {};
+	init_b();
+} });
+
+//#endregion
+//#region b.js
+var b_exports, import_c;
+var init_b = __esm({ "b.js"() {
+	b_exports = {};
+	import_c = __toESM(require_c());
+} });
+
+//#endregion
+//#region c.js
+var require_c = __commonJS({ "c.js"() {
+	await 0;
+} });
+
+//#endregion
+export default require_entry();

```