import('./b').then(ns => console.log(ns))
import(` + "`./b`" + `).then(ns => console.log(ns))