# Reason
1. not support glob
# Diff
## /out.js
### esbuild
```js
// src/a.ts
var require_a = __commonJS({
  "src/a.ts"(exports, module) {
    module.exports = "a";
  }
});

// src/b.ts
var require_b = __commonJS({
  "src/b.ts"(exports, module) {
    module.exports = "b";
  }
});

// require("./src/**/*") in entry.ts
var globRequire_src = __glob({
  "./src/a.ts": () => require_a(),
  "./src/b.ts": () => require_b()
});

// import("./src/**/*") in entry.ts
var globImport_src = __glob({
  "./src/a.ts": () => Promise.resolve().then(() => __toESM(require_a())),
  "./src/b.ts": () => Promise.resolve().then(() => __toESM(require_b()))
});

// entry.ts
var ab = Math.random() < 0.5 ? "a.ts" : "b.ts";
console.log({
  concat: {
    require: globRequire_src("./src/" + ab),
    import: globImport_src("./src/" + ab)
  },
  template: {
    require: globRequire_src(`./src/${ab}`),
    import: globImport_src(`./src/${ab}`)
  }
});
```
### rolldown
```js

//#region rolldown:runtime
var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, { get: (a, b) => (typeof require !== "undefined" ? require : a)[b] }) : x)(function(x) {
	if (typeof require !== "undefined") return require.apply(this, arguments);
	throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
});

//#region entry.ts
const ab = Math.random() < .5 ? "a.ts" : "b.ts";
console.log({
	concat: {
		require: __require("./src/" + ab),
		import: import("./src/" + ab)
	},
	template: {
		require: __require(`./src/${ab}`),
		import: import(`./src/${ab}`)
	}
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,17 @@
-var require_a = __commonJS({
-    "src/a.ts"(exports, module) {
-        module.exports = "a";
-    }
+var __require = (x => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
+    get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
+}) : x)(function (x) {
+    if (typeof require !== "undefined") return require.apply(this, arguments);
+    throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
 });
-var require_b = __commonJS({
-    "src/b.ts"(exports, module) {
-        module.exports = "b";
-    }
-});
-var globRequire_src = __glob({
-    "./src/a.ts": () => require_a(),
-    "./src/b.ts": () => require_b()
-});
-var globImport_src = __glob({
-    "./src/a.ts": () => Promise.resolve().then(() => __toESM(require_a())),
-    "./src/b.ts": () => Promise.resolve().then(() => __toESM(require_b()))
-});
-var ab = Math.random() < 0.5 ? "a.ts" : "b.ts";
+var ab = Math.random() < .5 ? "a.ts" : "b.ts";
 console.log({
     concat: {
-        require: globRequire_src("./src/" + ab),
-        import: globImport_src("./src/" + ab)
+        require: __require("./src/" + ab),
+        import: import("./src/" + ab)
     },
     template: {
-        require: globRequire_src(`./src/${ab}`),
-        import: globImport_src(`./src/${ab}`)
+        require: __require(`./src/${ab}`),
+        import: import(`./src/${ab}`)
     }
 });

```