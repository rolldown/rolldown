---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: 'Rolldown'
  text: 'Fast Rust-based bundler for JavaScript'
  tagline: 'with Rollup-compatible API'
  image:
    src: /rolldown-round.svg
    alt: Rolldown
  actions:
    - text: Why Rolldown?
      openVideoModal: true
    - theme: brand
      text: Get Started
      link: /guide/
    - theme: alt
      text: Contribute
      link: /contrib-guide/

features:
  - title: Speed of Rust
    icon:
      src: /ferris.svg
    details: |
      Rolldown handles tens of thousands of modules without breaking a sweat
  - title: Rollup Compatible
    icon:
      src: /rollup.svg
      width: 32px
      height: 32px
    details: |
      Familiar API & options<br>Rich plugin ecosystem
  - title: esbuild Feature Parity
    icon:
      src: /esbuild.svg
      width: 32px
      height: 32px
    details: |
      Built-in transforms, define, inject, minify, CSS bundling & more...
  - title: Designed for Vite
    icon:
      src: /vite.svg
      width: 32px
      height: 32px
    details: |
      Serving as the unified bundler in Vite in the near future
---

<h2 class="voidzero-lead">Brought to you by</h2>

<a class="voidzero" href="https://voidzero.dev/" target="_blank" title="voidzero.dev"></a>

<style>
:root {
  --vp-home-hero-name-color: transparent;
  --vp-home-hero-name-background: -webkit-linear-gradient(90deg, #FF5D13, #F0DB4F);
}

h2.voidzero-lead {
  text-align: center;
  padding-top: 60px;
}

.voidzero {
  display: block;
  width: 300px;
  height: 74px;
  margin: 30px auto -20px;
  background-image: url(https://voidzero.dev/logo.svg);
  background-repeat: no-repeat;
  background-size: auto 74px;
  background-position: center;
}

.dark .voidzero {
  background-image: url(https://voidzero.dev/logo-white.svg);
}
</style>

<script setup>
import { onMounted } from 'vue'

onMounted(() => {
  const urlParams = new URLSearchParams(window.location.search)
  if (urlParams.get('uwu') != null) {
    const img = document.querySelector('.VPHero .VPImage.image-src')
    img.src = '/rolldown-uwu.png'
    img.alt = 'Rolldown Kawaii Logo by @icarusgkx'
    img.style.maxWidth = '540px'
  }
})
</script>
