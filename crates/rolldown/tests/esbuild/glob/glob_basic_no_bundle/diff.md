# Diff
## /out.js
### esbuild
```js
const ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
  concat: {
    require: require("./src/" + ab),
    import: import("./src/" + ab)
  },
  template: {
    require: require(`./src/${ab}`),
    import: import(`./src/${ab}`)
  }
});
```
### rolldown
```js

//#region entry.js
const ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
	concat: {
		require: require("./src/" + ab),
		import: import("./src/" + ab)
	},
	template: {
		require: require(`./src/${ab}`),
		import: import(`./src/${ab}`)
	}
});

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-const ab = Math.random() < 0.5 ? "a.js" : "b.js";
+var ab = Math.random() < 0.5 ? "a.js" : "b.js";
 console.log({
     concat: {
         require: require("./src/" + ab),
         import: import("./src/" + ab)

```