# Reason
1. not support glob
# Diff
## /out/entry.js
### esbuild
```js
// require("./src/**/*.json") in entry.js
var globRequire_src_json = __glob({});

// import("./src/**/*.json") in entry.js
var globImport_src_json = __glob({});

// entry.js
var ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
  concat: {
    require: globRequire_src_json("./src/" + ab + ".json"),
    import: globImport_src_json("./src/" + ab + ".json")
  },
  template: {
    require: globRequire_src_json(`./src/${ab}.json`),
    import: globImport_src_json(`./src/${ab}.json`)
  }
});
```
### rolldown
```js

//#region entry.js
const ab = Math.random() < .5 ? "a.js" : "b.js";
console.log({
	concat: {
		require: require("./src/" + ab + ".json"),
		import: import("./src/" + ab + ".json")
	},
	template: {
		require: require(`./src/${ab}.json`),
		import: import(`./src/${ab}.json`)
	}
});

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,13 +1,11 @@
-var globRequire_src_json = __glob({});
-var globImport_src_json = __glob({});
-var ab = Math.random() < 0.5 ? "a.js" : "b.js";
+var ab = Math.random() < .5 ? "a.js" : "b.js";
 console.log({
     concat: {
-        require: globRequire_src_json("./src/" + ab + ".json"),
-        import: globImport_src_json("./src/" + ab + ".json")
+        require: require("./src/" + ab + ".json"),
+        import: import("./src/" + ab + ".json")
     },
     template: {
-        require: globRequire_src_json(`./src/${ab}.json`),
-        import: globImport_src_json(`./src/${ab}.json`)
+        require: require(`./src/${ab}.json`),
+        import: import(`./src/${ab}.json`)
     }
 });

```