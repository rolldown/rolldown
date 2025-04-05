// This file is inside your library
declare global {
  interface ImportMeta {
    hot: {
      accept: (callback: (exports: any) => void) => void;
    };
  }
  function render(): void;
}

export {};
