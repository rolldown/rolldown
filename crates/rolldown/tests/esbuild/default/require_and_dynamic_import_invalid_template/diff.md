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
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,41 +1,18 @@
-var globRequire;
-var init_ = __esm({
-    'require("./**/*") in entry.js'() {
-        globRequire = __glob({
-            "./entry.js": () => require_entry()
-        });
-    }
-});
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