const modules = import.meta.glob('./dir/**/*.js', { eager: true });

export { modules };
