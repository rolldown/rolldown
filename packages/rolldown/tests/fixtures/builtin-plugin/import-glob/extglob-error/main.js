// Extglob patterns like !(*.d.ts) are not supported — should produce a build error.
const modules = import.meta.glob('./dir/**/!(*.d.ts)');
export { modules };
