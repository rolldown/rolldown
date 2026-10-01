// The home page, assembled from the sections of the VoidZero marketing pages.
import { ArrowUpRight } from '@voidzero-dev/doc-kit-theme/components/icons.jsx';
import RiveAnimation from '@voidzero-dev/doc-kit-theme/components/RiveAnimation.jsx';
import { classNames } from '@voidzero-dev/doc-kit-theme/utils';

const PERFORMANCE = [
  { name: 'Rolldown', percentage: 4.01, time: '1.61s', primary: true },
  { name: 'esbuild', percentage: 4.24, time: '1.70s' },
  { name: 'rspack', percentage: 10.15, time: '4.07s' },
  { name: 'Rollup + esbuild', percentage: 100, time: '40.10s' },
];

/**
 * The home page.
 */
export default () => (
  <>
    <div class="wrapper wrapper--ticks grid w-full divide-x border-nickel md:grid-cols-2">
      <div class="flex flex-col items-center justify-between p-10 md:items-start">
        <div class="flex flex-col items-center gap-5 text-center md:items-start md:text-left">
          <a class="flex items-center gap-2" href="https://voidzero.dev" target="_blank">
            <span class="font-mono text-xs tracking-wide text-grey uppercase">By</span>
            <img src="/assets/brand/voidzero-light.svg" alt="VoidZero" class="h-2.5" />
          </a>
          <h1 class="max-w-[35rem] text-pretty text-white">
            Blazing Fast
            <br />
            Rust-based bundler for JavaScript
          </h1>
          <p class="max-w-[30rem] text-lg text-pretty text-white/70">
            with Rollup-compatible API and esbuild feature parity
          </p>
          <div class="mt-6 flex items-center gap-5">
            <a href="/guide/getting-started" class="button button--primary inline-block w-fit">
              <span>Get Started</span>
            </a>
            <a
              href="https://github.com/rolldown/rolldown"
              target="_blank"
              rel="noopener noreferrer"
              class="button inline-flex w-fit items-center gap-2"
            >
              <span>View on GitHub</span>
              <ArrowUpRight class="size-3" />
            </a>
          </div>
        </div>
        <div class="mt-8 flex w-fit items-center gap-2 rounded bg-slate px-3 py-1.5 md:mt-0">
          <a href="https://repl.rolldown.rs/" target="_blank">
            <figure class="project-icon gap-3">
              <img
                class="size-5"
                loading="lazy"
                src="/assets/brand/rolldown-icon-light.svg"
                alt="Rolldown"
              />
              <figcaption class="text-sm">Try Rolldown in the REPL</figcaption>
              <ArrowUpRight class="size-3 text-fire" />
            </figure>
          </a>
        </div>
      </div>

      <div class="flex min-h-[22rem] flex-col sm:min-h-[30rem]">
        <div
          class="relative flex h-full flex-col justify-center overflow-clip bg-[#ed4d01] bg-cover bg-center py-8 pl-6 sm:py-16 sm:pl-16"
          style={{ backgroundImage: "url('/assets/brand/rolldown/hero-background.jpg')" }}
        >
          <img
            src="/assets/brand/rolldown/hero-terminal.svg"
            width="587"
            height="405"
            alt="Rolldown terminal"
            class="h-full w-full object-contain"
          />
        </div>
        <a
          href="https://www.youtube.com/watch?v=RRjfm8cMveQ"
          target="_blank"
          class="group relative flex items-center gap-5 p-5"
        >
          <img
            src="/assets/brand/rolldown/rolldown-thumbnail.png"
            alt="Video thumbnail"
            class="aspect-[244/144] h-16 transition-[scale,opacity] group-hover:scale-105 group-hover:opacity-75"
          />
          <div>
            <h5 class="text-white">What is Rolldown</h5>
            <p class="text-base">Rolldown explained in 37 seconds</p>
          </div>
          <span class="absolute top-5 right-5 rounded bg-slate px-3 py-1 font-mono text-xs text-fire">
            0:37
          </span>
        </a>
      </div>
    </div>
    <section class="wrapper flex flex-col items-center justify-center gap-3 border-t px-5 py-14 text-center sm:px-10 sm:py-28">
      <h2 class="max-w-2xl text-center text-balance text-white">
        Performance + Features + Ecosystem
      </h2>
    </section>
    <section class="wrapper wrapper--ticks grid divide-x divide-y divide-nickel border-t lg:grid-cols-2">
      <div class="flex flex-col justify-between gap-3">
        <div class="flex flex-col gap-3 p-5 pb-0 sm:p-15 sm:pb-0">
          <h5 class="text-balance text-white sm:text-pretty">Speed of Rust</h5>
          <p class="text-pretty sm:max-w-[30rem]">
            Rolldown handles tens of thousands of modules without breaking a sweat
          </p>
        </div>
        <div class="flex flex-col gap-10 p-5 sm:p-15">
          <div class="flex flex-col gap-4">
            {PERFORMANCE.map(({ name, percentage, time, primary }) => (
              <div key={name} class="flex w-full items-center gap-3">
                <p
                  class={classNames(
                    'w-24 shrink-0 font-mono text-xs tracking-wide uppercase md:w-36',
                    primary ? 'text-white' : 'text-grey',
                  )}
                >
                  {name}
                </p>
                <div
                  class="relative h-3 w-full overflow-hidden rounded-xs bg-slate"
                  role="progressbar"
                  aria-label={name}
                >
                  <div
                    class={classNames(
                      'relative block h-full overflow-hidden rounded-xs bg-cover bg-center',
                      primary ? 'bg-wine' : 'bg-grey',
                    )}
                    style={{
                      width: `${percentage}%`,
                      backgroundImage: primary
                        ? "url('/assets/brand/rolldown/rolldown-rollup-background.jpg')"
                        : undefined,
                    }}
                  />
                </div>
                <p
                  class={classNames(
                    'shrink-0 font-mono text-xs tracking-wide uppercase',
                    primary ? 'text-white' : 'text-grey',
                  )}
                >
                  / {time}
                </p>
              </div>
            ))}
          </div>
          <div>
            <p class="text-sm">
              <a
                href="https://github.com/rolldown/benchmarks/tree/f3997c801b7d261e8aaa15d4affbf6730cc19884?tab=readme-ov-file#ubuntu-latest-updated-2025-12-21"
                target="_blank"
                class="font-medium text-white"
              >
                Benchmark
              </a>{' '}
              bundling <span class="font-medium text-white">19k</span> modules.
            </p>
            <p class="text-xs">
              10k React JSX components + 9k iconify JS files, with minification and source maps.
            </p>
          </div>
        </div>
      </div>

      <div class="flex flex-col justify-between gap-3 overflow-hidden border-r-0">
        <div class="flex flex-col gap-3 p-5 sm:p-15 sm:pb-10">
          <h5 class="text-white">Rollup Compatible</h5>
          <p class="max-w-[30rem] text-pretty">
            Familiar API &amp; options with a rich plugin ecosystem
          </p>
        </div>
        <div class="relative flex justify-center bg-[#ed4d01] px-5 pt-5 sm:px-20 sm:pt-10">
          <img
            class="absolute inset-0 h-full w-full object-cover"
            src="/assets/brand/rolldown/rolldown-rollup-background.jpg"
            alt=""
            inert
            loading="lazy"
          />
          <img
            src="/assets/brand/rolldown/rolldown-rollup.png"
            width="892"
            height="555"
            class="z-1 -mb-[60px]"
            inert
            loading="lazy"
            alt="rollup compatible"
          />
        </div>
      </div>

      <div class="flex flex-col justify-between gap-3">
        <div class="flex flex-col gap-3 p-5 pb-0 sm:p-15 sm:pb-0">
          <h5 class="text-white">Esbuild Feature Parity</h5>
          <p class="max-w-[26rem] text-pretty">
            Built-in transforms, define, inject, minify &amp; more...
          </p>
        </div>
        <div class="flex flex-1 items-center justify-center p-8 sm:p-15 sm:pt-10">
          <img
            src="/assets/brand/rolldown/rolldown-esbuild-parity.png"
            width="742"
            height="493"
            inert
            loading="lazy"
            alt="esbuild feature parity"
            class="max-h-60 w-full object-contain"
          />
        </div>
      </div>

      <div class="flex flex-col justify-between gap-3">
        <div class="flex flex-col gap-3 p-5 pb-0 sm:p-15 sm:pb-0">
          <h5 class="text-white">Designed for Vite</h5>
          <p class="max-w-[25rem] text-pretty">The unified bundler powering Vite 8+</p>
        </div>
        <RiveAnimation
          src="/assets/brand/rolldown/animations/640_x_300_design_for_vite.riv"
          width={640}
          height={300}
          class="mb-10"
        />
      </div>
    </section>
    <div class="wrapper wrapper--ticks border-t px-10 py-14 sm:py-30">
      <div class="flex flex-col items-center justify-between gap-8 text-center md:flex-row md:gap-20 md:pl-15 md:text-left">
        <div class="flex flex-col gap-3">
          <h3 class="max-w-xl text-balance text-white">Free &amp; open source</h3>
          <p class="max-w-lg text-balance text-white/70">
            Rolldown is free and open source, made possible by a full-time team and passionate
            open-source contributors.
          </p>
          <a href="/contribution-guide/" class="button mx-auto mt-8 w-fit md:mx-0">
            Contribute
          </a>
        </div>
        <a
          class="flex items-start justify-center gap-8 md:justify-start md:gap-12 md:pr-25"
          href="https://voidzero.dev"
          target="_blank"
        >
          <img
            src="/assets/brand/vite-by-voidzero.png"
            alt="Brought to you by VoidZero"
            class="mt-10 h-44 max-w-full object-contain md:mt-0"
          />
        </a>
      </div>
    </div>
    <section class="wrapper wrapper--ticks h-16 border-t sm:h-30" />
    <section class="wrapper" data-theme="dark">
      <div class="relative w-full">
        <img
          src="/assets/brand/rolldown/footer-background.jpg"
          alt=""
          inert
          loading="lazy"
          class="absolute inset-0 z-0 h-full w-full object-cover"
        />
        <div class="relative z-10 mx-auto flex w-full flex-col items-center justify-start gap-5 px-5 py-10 sm:w-2xl sm:px-0 md:py-30">
          <h2 class="text-center text-balance text-white drop-shadow-sm/70">
            Optimize your JavaScript code with Rolldown
          </h2>
          <p class="max-w-md text-center text-balance text-white drop-shadow-sm/70">
            Why do we still need bundlers?
          </p>
          <a href="/in-depth/why-bundlers" class="button button--white mt-5">
            Learn more
          </a>
        </div>
      </div>
    </section>
  </>
);
