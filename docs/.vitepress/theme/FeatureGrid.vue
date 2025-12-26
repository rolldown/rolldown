<script setup>
import RiveAnimation from "@components/shared/RiveAnimation.vue";
import vitePoweredAnimation from '@assets/rolldown/animations/640_x_300_design_for_vite.riv'
import PerformanceBar from "@components/shared/PerformanceBar.vue";
import performanceBarBg from '@assets/rolldown/rolldown-rollup-background.jpg';

const performanceData = [
  {
    name: 'Rolldown',
    percentage: 4.01,
    time: '1.61s',
    isPrimary: true
  },
  {
    name: 'esbuild',
    percentage: 4.24,
    time: '1.70s'
  },
  {
    name: 'rspack',
    percentage: 10.15,
    time: '4.07s',
  },
  {
    name: 'Rollup + esbuild',
    percentage: 100,
    time: '40.10s'
  }
]
</script>

<template>
  <section class="wrapper wrapper--ticks border-t grid lg:grid-cols-2 divide-x divide-y divide-nickel">
    <div class="flex flex-col gap-3 justify-between">
      <div class="p-5 sm:p-15 pb-0 sm:pb-0 flex flex-col gap-3">
        <h5 class="text-balance sm:text-pretty text-white">Speed of Rust</h5>
        <p class="sm:max-w-[30rem] text-pretty">Rolldown handles tens of thousands of modules without breaking a sweat
        </p>
      </div>
      <div class="p-5 sm:p-15 flex flex-col gap-10">
        <div class="flex flex-col gap-4">
          <PerformanceBar v-for="item in performanceData" :key="item.name" :data="item"
            :background-image="performanceBarBg" />
        </div>
        <div>
          <p class="text-sm"><a
              href="https://github.com/rolldown/benchmarks/tree/f3997c801b7d261e8aaa15d4affbf6730cc19884?tab=readme-ov-file#ubuntu-latest-updated-2025-12-21"
              target="_blank" class="text-white font-medium">Benchmark</a> bundling <span
              class="text-white font-medium">19k</span>
            modules.</p>
          <p class="text-xs">10k React JSX components + 9k iconify JS files, with minification and source maps.</p>
        </div>
      </div>
    </div>

    <div class="flex flex-col gap-3 border-r-0 justify-between overflow-hidden">
      <div class="p-5 sm:p-15 sm:pb-10 flex flex-col gap-3">
        <h5 class="text-white">Rollup Compatible</h5>
        <p class="max-w-[30rem] text-pretty">Familiar API & options with a rich plugin ecosystem</p>
      </div>
      <div class="card-bg px-5 sm:px-20 pt-5 sm:pt-10 flex justify-center">
        <img src="@assets/rolldown/rolldown-rollup.png" class="-mb-[60px]" inert loading="lazy" alt="rollup compatible">
      </div>
    </div>

    <div class="flex flex-col gap-3 justify-between">
      <div class="p-5 sm:p-15 pb-0 sm:pb-0 flex flex-col gap-3">
        <h5 class="text-white">Esbuild Feature Parity</h5>
        <p class="max-w-[26rem] text-pretty">Built-in transforms, define, inject, minify, CSS bundling & more...</p>
      </div>
      <div class="flex-1 flex items-center justify-center p-8 sm:p-15 sm:pt-10">
        <img src="@assets/rolldown/rolldown-esbuild-parity.png" inert loading="lazy" alt="jest compatible"
          class="w-full max-h-60 object-contain">
      </div>
    </div>

    <div class="flex flex-col gap-3 justify-between">
      <div class="p-5 sm:p-15 pb-0 sm:pb-0 flex flex-col gap-3">
        <h5 class="text-white">Designed for Vite</h5>
        <p class="max-w-[25rem] text-pretty">The unified bundler powering Vite 8+</p>
      </div>
      <RiveAnimation :desktop-src="vitePoweredAnimation" :desktop-width="640" :desktop-height="300"
        canvas-class="w-full mb-10" />
    </div>
  </section>
</template>

<style scoped>
.card-bg {
  background-image: url('@assets/rolldown/rolldown-rollup-background.jpg');
  background-size: cover;
  background-position: center;
}
</style>
