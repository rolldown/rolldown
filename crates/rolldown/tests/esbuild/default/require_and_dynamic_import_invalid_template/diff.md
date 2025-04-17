# Reason
1. not support invalid template
# Diff
## /out.js
### esbuild
```js
// require("./**/*") in entry.js
var globRequire;
var init_ = __esm({
  'require("./**/*") in entry.js'() {
    globRequire = __glob({
      "./entry.js": () => require_entry()
    });
  }
});

// import("./**/*") in entry.js
var globImport;
var init_2 = __esm({
  'import("./**/*") in entry.js'() {
    globImport = __glob({
      "./entry.js": () => Promise.resolve().then(() => __toESM(require_entry()))
    });
  }
});

// entry.js
var require_entry = __commonJS({
  "entry.js"() {
    init_();
    init_2();
    __require(tag`./b`);
    globRequire(`./${b}`);
    try {
      __require(tag`./b`);
      globRequire(`./${b}`);
    } catch {
    }
    (async () => {
      import(tag`./b`);
      globImport(`./${b}`);
      await import(tag`./b`);
      await globImport(`./${b}`);
      try {
        import(tag`./b`);
        globImport(`./${b}`);
        await import(tag`./b`);
        await globImport(`./${b}`);
      } catch {
      }
    })();
  }
});
export default require_entry();
```
### rolldown
```js

//#region rolldown:runtime
var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, { get: (a, b$1) => (typeof require !== "undefined" ? require : a)[b$1] }) : x)(function(x) {
	if (typeof require !== "undefined") return require.apply(this, arguments);
	throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
});

//#region entry.js
__require(tag`./b`);
__require(`./${b}`);
try {
	__require(tag`./b`);
	__require(`./${b}`);
} catch {}
(async () => {
	import(tag`./b`);
	import(`./${b}`);
	await import(tag`./b`);
	await import(`./${b}`);
	try {
		import(tag`./b`);
		import(`./${b}`);
		await import(tag`./b`);
		await import(`./${b}`);
	} catch {}
})();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,41 +1,24 @@
-var globRequire;
-var init_ = __esm({
-    'require("./**/*") in entry.js'() {
-        globRequire = __glob({
-            "./entry.js": () => require_entry()
-        });
-    }
+var __require = (x => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
+    get: (a, b$1) => (typeof require !== "undefined" ? require : a)[b$1]
+}) : x)(function (x) {
+    if (typeof require !== "undefined") return require.apply(this, arguments);
+    throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
 });
-var globImport;
-var init_2 = __esm({
-    'import("./**/*") in entry.js'() {
-        globImport = __glob({
-            "./entry.js": () => Promise.resolve().then(() => __toESM(require_entry()))
-        });
-    }
-});
-var require_entry = __commonJS({
-    "entry.js"() {
-        init_();
-        init_2();
-        __require(tag`./b`);
-        globRequire(`./${b}`);
-        try {
-            __require(tag`./b`);
-            globRequire(`./${b}`);
-        } catch {}
-        (async () => {
-            import(tag`./b`);
-            globImport(`./${b}`);
-            await import(tag`./b`);
-            await globImport(`./${b}`);
-            try {
-                import(tag`./b`);
-                globImport(`./${b}`);
-                await import(tag`./b`);
-                await globImport(`./${b}`);
-            } catch {}
-        })();
-    }
-});
-export default require_entry();
+__require(tag`./b`);
+__require(`./${b}`);
+try {
+    __require(tag`./b`);
+    __require(`./${b}`);
+} catch {}
+(async () => {
+    import(tag`./b`);
+    import(`./${b}`);
+    await import(tag`./b`);
+    await import(`./${b}`);
+    try {
+        import(tag`./b`);
+        import(`./${b}`);
+        await import(tag`./b`);
+        await import(`./${b}`);
+    } catch {}
+})();

```