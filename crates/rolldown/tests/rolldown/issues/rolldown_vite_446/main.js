const foo = "foo";

export const authURL = import.meta.env.VITE_AUTH_URL ?? window.settings?.authURL;
