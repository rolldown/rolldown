import("./c").then((item) => {
  console.log('item', item.default()())
});



import("./a").then(console.log);

