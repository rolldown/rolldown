let rejection;

const onUnhandledRejection = (error) => {
  rejection = error;
};

process.on('unhandledRejection', onUnhandledRejection);

try {
  const main = await import('./dist/main.js');
  await main.loadSupported();
  await new Promise((resolve) => setTimeout(resolve, 50));
} finally {
  process.off('unhandledRejection', onUnhandledRejection);
}

if (rejection) {
  throw rejection;
}
