export const basic = import.meta.glob('../dir/*.js');

export const withBase = import.meta.glob('./dir/*.js', {
  base: '/',
});

export const external = import.meta.glob('../basic/dir/*.js', {
  base: '/',
});
