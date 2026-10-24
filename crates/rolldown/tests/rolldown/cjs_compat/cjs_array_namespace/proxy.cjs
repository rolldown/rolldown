module.exports = new Proxy(Object.assign([1, '2'], { foo: 'bar' }), {
  ownKeys: (target) => Reflect.ownKeys(target).reverse(),
});
