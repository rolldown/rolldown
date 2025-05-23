// should only include a in d1.js  
export const { a } = await import('./d1.js')
// should include all in d2.js
export default await import('./d2.js')

// should include all in d3.js
export const d3 =  await import('./d3.js')

// should include all in d4.js
export const d4 =  () => import('./d4.js')

// should include all in d5.js
export const d5 =  () => import('./d5.js').then(mod => {
  mod.a
})
