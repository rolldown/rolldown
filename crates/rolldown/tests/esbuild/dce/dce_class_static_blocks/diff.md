# Diff
## /out.js
### esbuild
```js
// entry.ts
var A_keep = class {
  static {
    foo;
  }
};
var B_keep = class {
  static {
    this.foo;
  }
};
var C_keep = class {
  static {
    try {
      foo;
    } catch {
    }
  }
};
var D_keep = class {
  static {
    try {
    } finally {
      foo;
    }
  }
};
```
### rolldown
```js

//#region entry.ts
var A_keep = class {
	static {
		foo;
	}
};
var B_keep = class {
	static {
		this.foo;
	}
};
var C_keep = class {
	static {
		try {
			foo;
		} catch {}
	}
};
var D_keep = class {
	static {
		{
			foo;
		}
	}
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -16,9 +16,9 @@
     }
 };
 var D_keep = class {
     static {
-        try {} finally {
+        {
             foo;
         }
     }
 };

```