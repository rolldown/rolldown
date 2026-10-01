import NavBar from '../components/NavBar.jsx';

/**
 * The page shown for URLs without a page.
 */
export default ({ metadata }) => (
  <div class="flex min-h-screen flex-col">
    <NavBar metadata={metadata} />
    <main class="px-6 pt-16 pb-24 text-center md:px-8 md:pt-24 md:pb-[168px]">
      <p class="text-[64px]/16 font-semibold">404</p>
      <h1 class="pt-3 font-sans text-[20px]/5 font-bold tracking-[2px]">PAGE NOT FOUND</h1>
      <div class="mx-auto mt-6 mb-[18px] h-px w-16 bg-divider" />
      <blockquote class="mx-auto max-w-64 text-[14px] font-medium text-text-2">
        But if you don't change your direction, and if you keep looking, you may end up where you
        are heading.
      </blockquote>
      <div class="pt-5">
        <a
          class="inline-block rounded-2xl border border-brand px-4 py-[3px] text-[14px] font-medium text-brand"
          href="/"
          aria-label="go to home"
        >
          Take me home
        </a>
      </div>
    </main>
  </div>
);
