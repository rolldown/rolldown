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
let keep1 = class {
	[x] = "x";
};
let keep2 = class {
	[x]() {}
};
let keep3 = class {
	get [x]() {}
};
let keep4 = class {
	set [x](_) {}
};
let keep5 = class {
	async [x]() {}
};
let keep6 = class {
	[{ toString() {} }] = "x";
};

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,20 +1,20 @@
-let keep1 = class {
+var keep1 = class {
     [x] = "x";
 };
-let keep2 = class {
+var keep2 = class {
     [x]() {}
 };
-let keep3 = class {
+var keep3 = class {
     get [x]() {}
 };
-let keep4 = class {
+var keep4 = class {
     set [x](_) {}
 };
-let keep5 = class {
+var keep5 = class {
     async [x]() {}
 };
-let keep6 = class {
+var keep6 = class {
     [{
         toString() {}
     }] = "x";
 };

```