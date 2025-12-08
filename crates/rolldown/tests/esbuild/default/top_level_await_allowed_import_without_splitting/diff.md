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
// HIDDEN [rolldown:runtime]
//#region c.js
var require_c = /* @__PURE__ */ __commonJSMin((() => {
	await 0;
}));

//#endregion
//#region b.js
var b_exports = {};
var import_c;
var init_b = __esmMin((async () => {
	import_c = require_c();
}));

//#endregion
//#region a.js
var a_exports = {};
var init_a = __esmMin((async () => {
	await init_b();
}));

//#endregion
//#region entry.js
var require_entry = /* @__PURE__ */ __commonJSMin((() => {
	init_a().then(() => a_exports);
	init_b().then(() => b_exports);
	Promise.resolve().then(() => /* @__PURE__ */ __toESM(require_c()));
	Promise.resolve().then(() => /* @__PURE__ */ __toESM(require_entry()));
	await 0;
}));

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,33 @@
-var c_exports = {};
-var init_c = __esm({
-    async "c.js"() {
-        await 0;
-    }
-});
+// HIDDEN [rolldown:runtime]
+//#region c.js
+var require_c = /* @__PURE__ */ __commonJSMin((() => {
+	await 0;
+}));
+
+//#endregion
+//#region b.js
 var b_exports = {};
-var init_b = __esm({
-    async "b.js"() {
-        await init_c();
-    }
-});
+var import_c;
+var init_b = __esmMin((async () => {
+	import_c = require_c();
+}));
+
+//#endregion
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
+var init_a = __esmMin((async () => {
+	await init_b();
+}));
+
+//#endregion
+//#region entry.js
+var require_entry = /* @__PURE__ */ __commonJSMin((() => {
+	init_a().then(() => a_exports);
+	init_b().then(() => b_exports);
+	Promise.resolve().then(() => /* @__PURE__ */ __toESM(require_c()));
+	Promise.resolve().then(() => /* @__PURE__ */ __toESM(require_entry()));
+	await 0;
+}));
+
+//#endregion
+export default require_entry();

```