# Reason
1. not support glob
# Diff
## /out.js
### esbuild
```js
// src/file-a.js
var require_file_a = __commonJS({
  "src/file-a.js"(exports, module) {
    module.exports = "a";
  }
});

// src/file-b.js
var require_file_b = __commonJS({
  "src/file-b.js"(exports, module) {
    module.exports = "b";
  }
});

// src/nested/dir/file-a.js
var require_file_a2 = __commonJS({
  "src/nested/dir/file-a.js"(exports, module) {
    module.exports = "a";
  }
});

// src/nested/dir/file-b.js
var require_file_b2 = __commonJS({
  "src/nested/dir/file-b.js"(exports, module) {
    module.exports = "b";
  }
});

// require("./src/**/*.js") in entry.js
var globRequire_src_js = __glob({
  "./src/file-a.js": () => require_file_a(),
  "./src/file-b.js": () => require_file_b(),
  "./src/nested/dir/file-a.js": () => require_file_a2(),
  "./src/nested/dir/file-b.js": () => require_file_b2()
});

// import("./src/**/*.js") in entry.js
var globImport_src_js = __glob({
  "./src/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a())),
  "./src/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b())),
  "./src/nested/dir/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a2())),
  "./src/nested/dir/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b2()))
});

// entry.js
var ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
  concat: {
    require: globRequire_src_js("./src/" + ab + ".js"),
    import: globImport_src_js("./src/" + ab + ".js")
  },
  template: {
    require: globRequire_src_js(`./src/${ab}.js`),
    import: globImport_src_js(`./src/${ab}.js`)
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

//#region entry.js
const ab = Math.random() < .5 ? "a.js" : "b.js";
console.log({
	concat: {
		require: __require("./src/" + ab + ".js"),
		import: import("./src/" + ab + ".js")
	},
	template: {
		require: __require(`./src/${ab}.js`),
		import: import(`./src/${ab}.js`)
	}
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,43 +1,17 @@
-var require_file_a = __commonJS({
-    "src/file-a.js"(exports, module) {
-        module.exports = "a";
-    }
+var __require = (x => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
+    get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
+}) : x)(function (x) {
+    if (typeof require !== "undefined") return require.apply(this, arguments);
+    throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
 });
-var require_file_b = __commonJS({
-    "src/file-b.js"(exports, module) {
-        module.exports = "b";
-    }
-});
-var require_file_a2 = __commonJS({
-    "src/nested/dir/file-a.js"(exports, module) {
-        module.exports = "a";
-    }
-});
-var require_file_b2 = __commonJS({
-    "src/nested/dir/file-b.js"(exports, module) {
-        module.exports = "b";
-    }
-});
-var globRequire_src_js = __glob({
-    "./src/file-a.js": () => require_file_a(),
-    "./src/file-b.js": () => require_file_b(),
-    "./src/nested/dir/file-a.js": () => require_file_a2(),
-    "./src/nested/dir/file-b.js": () => require_file_b2()
-});
-var globImport_src_js = __glob({
-    "./src/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a())),
-    "./src/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b())),
-    "./src/nested/dir/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a2())),
-    "./src/nested/dir/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b2()))
-});
-var ab = Math.random() < 0.5 ? "a.js" : "b.js";
+var ab = Math.random() < .5 ? "a.js" : "b.js";
 console.log({
     concat: {
-        require: globRequire_src_js("./src/" + ab + ".js"),
-        import: globImport_src_js("./src/" + ab + ".js")
+        require: __require("./src/" + ab + ".js"),
+        import: import("./src/" + ab + ".js")
     },
     template: {
-        require: globRequire_src_js(`./src/${ab}.js`),
-        import: globImport_src_js(`./src/${ab}.js`)
+        require: __require(`./src/${ab}.js`),
+        import: import(`./src/${ab}.js`)
     }
 });

```