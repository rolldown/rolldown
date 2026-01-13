<template>
  <div class="wrapper wrapper--ticks grid md:grid-cols-2 w-full border-nickel divide-x">
    <div class="flex flex-col p-10 justify-between items-center md:items-start">
      <div class="flex flex-col gap-5 text-center md:text-left items-center md:items-start">
        <a class="flex items-center gap-2" href="https://voidzero.dev" target="_blank">
          <span class="text-grey text-xs font-mono uppercase tracking-wide">By</span>
          <img src="@assets/logos/voidzero-light.svg" alt="VoidZero" class="h-2.5" />
        </a>
        <h1 class="text-white text-pretty max-w-[35rem]">
          Blazing Fast<br />Rust-based bundler for JavaScript
        </h1>
        <p class="text-white/70 text-lg max-w-[30rem] text-pretty">
          with Rollup-compatible API and esbuild feature parity
        </p>
        <div class="flex items-center gap-5 mt-6">
          <a href="/guide/getting-started" class="button button--primary inline-block w-fit">
            <span>Get Started</span>
          </a>
          <a
            href="https://github.com/rolldown/rolldown"
            target="_blank"
            rel="noopener noreferrer"
            class="button inline-block w-fit"
          >
            View on GitHub
          </a>
        </div>
      </div>
      <div class="px-3 py-1.5 bg-slate rounded w-fit flex gap-2 items-center mt-8 md:mt-0">
        <a href="https://repl.rolldown.rs/" target="_blank">
          <figure class="project-icon gap-3">
            <img
              class="size-5"
              loading="lazy"
              src="@assets/icons/rolldown-light.svg"
              alt="Rolldown"
            />
            <figcaption class="text-sm">Try Rolldown in the REPL</figcaption>
            <svg class="size-3" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path
                d="M3.18228 2.81797L9.54624 2.81797L9.54624 9.18193"
                stroke="#FF5500"
                stroke-width="1.35"
                stroke-linejoin="round"
              />
              <path
                d="M9.5459 2.81799L3.18194 9.18195"
                stroke="#FF5500"
                stroke-width="1.35"
                stroke-linejoin="round"
              />
            </svg>
          </figure>
        </a>
      </div>
    </div>
    <div class="flex flex-col min-h-[22rem] sm:min-h-[30rem]">
      <div
        class="relative pl-6 sm:pl-16 h-full flex flex-col justify-center overflow-clip py-8 sm:py-16 hero-background"
      >
        <img
          src="@assets/rolldown/hero-terminal.svg"
          width="587"
          height="405"
          alt="Rolldown terminal"
          class="w-full h-full object-contain"
        />
      </div>
      <a
        href="https://www.youtube.com/watch?v=RRjfm8cMveQ"
        target="_blank"
        class="p-5 flex gap-5 items-center relative group"
      >
        <img
          src="@assets/rolldown/rolldown-thumbnail.png"
          alt="Video thumbnail"
          class="h-16 aspect-[244/144] group-hover:opacity-75 group-hover:scale-105 transition-[scale,opacity]"
        />
        <div>
          <h5 class="text-white">What is Rolldown</h5>
          <p class="text-base">Rolldown explained in 37 seconds</p>
        </div>
        <span class="absolute top-5 right-5 text-fire bg-slate rounded px-3 py-1 text-xs font-mono"
          >0:37</span
        >
      </a>
    </div>
  </div>
</template>

<style scoped>
.hero-background {
  background-image: url('@assets/rolldown/hero-background.jpg');
  background-color: #ed4d01;
  background-size: cover;
  background-position: center;
}
</style>
