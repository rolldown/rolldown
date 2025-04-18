# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
export default [
  async () => {
    try {
      for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
        x = temp.value;
        z(x);
      }
    } catch (temp) {
      error = [temp];
    } finally {
      try {
        more && (temp = iter.return) && await temp.call(iter);
      } finally {
        if (error)
          throw error[0];
      }
    }
  },
  async () => {
    try {
      for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
        x.y = temp.value;
        z(x);
      }
    } catch (temp) {
      error = [temp];
    } finally {
      try {
        more && (temp = iter.return) && await temp.call(iter);
      } finally {
        if (error)
          throw error[0];
      }
    }
  },
  async () => {
    try {
      for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
        let x2 = temp.value;
        z(x2);
      }
    } catch (temp) {
      error = [temp];
    } finally {
      try {
        more && (temp = iter.return) && await temp.call(iter);
      } finally {
        if (error)
          throw error[0];
      }
    }
  },
  async () => {
    try {
      for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
        const x2 = temp.value;
        z(x2);
      }
    } catch (temp) {
      error = [temp];
    } finally {
      try {
        more && (temp = iter.return) && await temp.call(iter);
      } finally {
        if (error)
          throw error[0];
      }
    }
  },
  async () => {
    try {
      label: for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
        const x2 = temp.value;
        break label;
      }
    } catch (temp) {
      error = [temp];
    } finally {
      try {
        more && (temp = iter.return) && await temp.call(iter);
      } finally {
        if (error)
          throw error[0];
      }
    }
  },
  async () => {
    try {
      label: for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
        const x2 = temp.value;
        continue label;
      }
    } catch (temp) {
      error = [temp];
    } finally {
      try {
        more && (temp = iter.return) && await temp.call(iter);
      } finally {
        if (error)
          throw error[0];
      }
    }
  }
];
```
### rolldown
```js
//#region entry.js
var entry_default = [
	async () => {
		for await (x of y) z(x);
	},
	async () => {
		for await (x.y of y) z(x);
	},
	async () => {
		for await (let x$1 of y) z(x$1);
	},
	async () => {
		for await (const x$1 of y) z(x$1);
	}
];

//#endregion
export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,91 +1,10 @@
-export default [async () => {
-    try {
-        for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
-            x = temp.value;
-            z(x);
-        }
-    } catch (temp) {
-        error = [temp];
-    } finally {
-        try {
-            more && (temp = iter.return) && await temp.call(iter);
-        } finally {
-            if (error) throw error[0];
-        }
-    }
+var entry_default = [async () => {
+    for await (x of y) z(x);
 }, async () => {
-    try {
-        for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
-            x.y = temp.value;
-            z(x);
-        }
-    } catch (temp) {
-        error = [temp];
-    } finally {
-        try {
-            more && (temp = iter.return) && await temp.call(iter);
-        } finally {
-            if (error) throw error[0];
-        }
-    }
+    for await (x.y of y) z(x);
 }, async () => {
-    try {
-        for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
-            let x2 = temp.value;
-            z(x2);
-        }
-    } catch (temp) {
-        error = [temp];
-    } finally {
-        try {
-            more && (temp = iter.return) && await temp.call(iter);
-        } finally {
-            if (error) throw error[0];
-        }
-    }
+    for await (let x$1 of y) z(x$1);
 }, async () => {
-    try {
-        for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
-            const x2 = temp.value;
-            z(x2);
-        }
-    } catch (temp) {
-        error = [temp];
-    } finally {
-        try {
-            more && (temp = iter.return) && await temp.call(iter);
-        } finally {
-            if (error) throw error[0];
-        }
-    }
-}, async () => {
-    try {
-        label: for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
-            const x2 = temp.value;
-            break label;
-        }
-    } catch (temp) {
-        error = [temp];
-    } finally {
-        try {
-            more && (temp = iter.return) && await temp.call(iter);
-        } finally {
-            if (error) throw error[0];
-        }
-    }
-}, async () => {
-    try {
-        label: for (var iter = __forAwait(y), more, temp, error; more = !(temp = await iter.next()).done; more = false) {
-            const x2 = temp.value;
-            continue label;
-        }
-    } catch (temp) {
-        error = [temp];
-    } finally {
-        try {
-            more && (temp = iter.return) && await temp.call(iter);
-        } finally {
-            if (error) throw error[0];
-        }
-    }
+    for await (const x$1 of y) z(x$1);
 }];
+export {entry_default as default};

```