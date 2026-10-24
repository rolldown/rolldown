const { proxy, revoke } = Proxy.revocable({}, {
  ownKeys(target) {
    revoke();
    return Reflect.ownKeys(target);
  },
});
module.exports = proxy;
