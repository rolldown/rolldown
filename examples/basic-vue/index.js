// const {
// 		foo,
// 	} = await import("./lib.js"),
// 	c = 10000;
import("./lib.js").then(({foo}) =>  {
  console.log(foo)
})
// export { foo };
