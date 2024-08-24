// const {
// 		foo,
// 	} = await import("./lib.js"),
// 	c = 10000;
// import("./lib.js").then(({foo}) =>  {
//   console.log(foo)
// });
const {foo} = await import('./lib.js');

const b = (await import("./b.js")).b;
// import("./a.js").then(({a}) => {
//   console.log(a)
// });
console.log(foo, b)
// export {foo, }
// export { foo };
