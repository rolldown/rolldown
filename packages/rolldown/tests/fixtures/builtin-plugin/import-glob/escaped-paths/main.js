const eager = import.meta.glob('./dir/*.eager.js', { eager: true });
const lazy = import.meta.glob('./dir/*.lazy.js');
const keys = Object.keys(import.meta.glob('./dir/*.keys.js'));

export { eager, lazy, keys };
