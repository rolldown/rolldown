const modules = import.meta.glob('./linked/*/components/*.js', { eager: true });

export { modules };
