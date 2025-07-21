# Diff
## /out.js
### esbuild
```js
let keep1 = class {
  [x] = "x";
};
let keep2 = class {
  [x]() {
  }
};
let keep3 = class {
  get [x]() {
  }
};
let keep4 = class {
  set [x](_) {
  }
};
let keep5 = class {
  async [x]() {
  }
};
let keep6 = class {
  [{ toString() {
  } }] = "x";
};
```
### rolldown
```js
//#region entry.js
(class {
	[x] = "x";
});
(class {
	[x]() {}
});
(class {
	get [x]() {}
});
(class {
	set [x](_) {}
});
(class {
	async [x]() {}
});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,20 +1,15 @@
-let keep1 = class {
+(class {
     [x] = "x";
-};
-let keep2 = class {
+});
+(class {
     [x]() {}
-};
-let keep3 = class {
+});
+(class {
     get [x]() {}
-};
-let keep4 = class {
+});
+(class {
     set [x](_) {}
-};
-let keep5 = class {
+});
+(class {
     async [x]() {}
-};
-let keep6 = class {
-    [{
-        toString() {}
-    }] = "x";
-};
+});

```