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

// require("./src/file-*.js") in entry.js
var globRequire_src_file_js = __glob({
  "./src/file-a.js": () => require_file_a(),
  "./src/file-b.js": () => require_file_b()
});

// import("./src/file-*.js") in entry.js
var globImport_src_file_js = __glob({
  "./src/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a())),
  "./src/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b()))
});

// entry.js
var ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
  concat: {
    require: globRequire_src_file_js("./src/file-" + ab + ".js"),
    import: globImport_src_file_js("./src/file-" + ab + ".js")
  },
  template: {
    require: globRequire_src_file_js(`./src/file-${ab}.js`),
    import: globImport_src_file_js(`./src/file-${ab}.js`)
  }
});
```
### rolldown
```js

//#region entry.js
const ab = Math.random() < .5 ? "a.js" : "b.js";
console.log({
	concat: {
		require: require("./src/file-" + ab + ".js"),
		import: import("./src/file-" + ab + ".js")
	},
	template: {
		require: require(`./src/file-${ab}.js`),
		import: import(`./src/file-${ab}.js`)
	}
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,11 @@
-var require_file_a = __commonJS({
-    "src/file-a.js"(exports, module) {
-        module.exports = "a";
-    }
-});
-var require_file_b = __commonJS({
-    "src/file-b.js"(exports, module) {
-        module.exports = "b";
-    }
-});
-var globRequire_src_file_js = __glob({
-    "./src/file-a.js": () => require_file_a(),
-    "./src/file-b.js": () => require_file_b()
-});
-var globImport_src_file_js = __glob({
-    "./src/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a())),
-    "./src/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b()))
-});
-var ab = Math.random() < 0.5 ? "a.js" : "b.js";
+var ab = Math.random() < .5 ? "a.js" : "b.js";
 console.log({
     concat: {
-        require: globRequire_src_file_js("./src/file-" + ab + ".js"),
-        import: globImport_src_file_js("./src/file-" + ab + ".js")
+        require: require("./src/file-" + ab + ".js"),
+        import: import("./src/file-" + ab + ".js")
     },
     template: {
-        require: globRequire_src_file_js(`./src/file-${ab}.js`),
-        import: globImport_src_file_js(`./src/file-${ab}.js`)
+        require: require(`./src/file-${ab}.js`),
+        import: import(`./src/file-${ab}.js`)
     }
 });

```