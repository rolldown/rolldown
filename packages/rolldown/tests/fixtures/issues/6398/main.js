import nodeAssert from 'node:assert'
import { cjsDepStar } from 'dep'

// If `lib/lib.js` is marked as `package.json#type: "module"` by rolldown,
// then `cjsDepStar.default` should point to `cjsDepStar` itself
nodeAssert.equal(cjsDepStar.value, 'cjs-dep')
nodeAssert.equal(cjsDepStar.default.value, 'cjs-dep')