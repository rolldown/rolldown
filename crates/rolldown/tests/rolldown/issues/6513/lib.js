const remoteProvider = async function() {
  const mod = await import('./dynamic.js');
  return mod;
};

exports.defaultProvider = remoteProvider;
