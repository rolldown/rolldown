import.meta.glob('./dir/*', { eager: true }).then((res) => {
  console.log(res) // expect: { "./dir/a.js": Module { default: "a" } }
})
//
import.meta.glob('/src/dir/*', { eager: true }).then((res) => {
  console.log(res) // expect: { "./dir/a.js": Module { default: "a" } }
})

import.meta.glob('./dir/*.js', { eager: true }).then((res) => {
  console.log(res) // expect: { "./dir/a.js": Module { default: "a" } }
})
//
