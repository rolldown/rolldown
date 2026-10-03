import { lastUpdated } from '#theme/data';

import navigation from '../../navigation.json' with { type: 'json' };
import { SquarePen } from '@voidzero-dev/doc-kit-theme/components/icons.jsx';
import LocalNav from '@voidzero-dev/doc-kit-theme/components/LocalNav.jsx';
import Outline, { outlineHeadings } from '@voidzero-dev/doc-kit-theme/components/Outline.jsx';
import SideBar from '@voidzero-dev/doc-kit-theme/components/SideBar.jsx';
import {
  classNames,
  findSidebar,
  flattenSidebar,
  isCurrent,
} from '@voidzero-dev/doc-kit-theme/utils';

import NavBar from '../components/NavBar.jsx';
import { referenceSidebar } from '../reference.mjs';

const formatDate = new Intl.DateTimeFormat('en-US', {
  dateStyle: 'medium',
  timeStyle: 'long',
  timeZone: 'UTC',
});

const PagerLink = ({ page, next }) => (
  <a
    class={classNames(
      'block size-full rounded-lg border border-divider px-4 pt-[11px] pb-[13px] transition-[border-color] duration-250 hover:border-brand',
      next && 'ml-auto text-right',
    )}
    href={page.link}
  >
    <span class="block text-[12px]/5 font-medium text-text-2">
      {next ? 'Next page' : 'Previous page'}
    </span>
    <span class="block text-[14px]/5 font-medium text-brand">{page.text}</span>
  </a>
);

/**
 * The end of a documentation page: a link to edit its source, when it last
 * changed, and links to the previous and next pages of its sidebar.
 */
const DocFooter = ({ metadata, sidebar }) => {
  const pages = flattenSidebar(sidebar);
  const index = pages.findIndex(({ link }) => isCurrent(link, metadata.path));
  const [previous, next] = index === -1 ? [] : [pages[index - 1], pages[index + 1]];

  // The generated reference has no source page to edit
  const edit =
    !metadata.path.startsWith('/reference/') &&
    navigation.editLink.pattern.replace(
      ':path',
      `${metadata.path.slice(1)}.${metadata.mdx ? 'mdx' : 'md'}`,
    );
  const updated = lastUpdated[metadata.path];

  if (!edit && !previous && !next) {
    return null;
  }

  return (
    <footer class="mt-16">
      {edit && (
        <div class="pb-[18px] sm:flex sm:items-center sm:justify-between sm:pb-3.5">
          <a
            class="flex items-center gap-2 text-[14px]/8 font-medium text-brand transition-colors duration-250"
            href={edit}
          >
            <SquarePen class="size-3.5" />
            Edit this page on GitHub
          </a>
          {updated && (
            <p class="text-[14px]/6 font-medium text-text-2 sm:leading-8">
              Last updated:{' '}
              <time dateTime={new Date(updated).toISOString()}>{formatDate.format(updated)}</time>
            </p>
          )}
        </div>
      )}

      {(previous || next) && (
        <nav
          class="grid gap-y-2 border-t border-divider pt-6 sm:grid-cols-2 sm:gap-x-4"
          aria-label="Pager"
        >
          <div>{previous && <PagerLink page={previous} />}</div>
          <div>{next && <PagerLink page={next} next />}</div>
        </nav>
      )}
    </footer>
  );
};

/**
 * A documentation page: sidebar, content and outline.
 */
export default ({ metadata, headings, children }) => {
  const sidebar = findSidebar(
    { ...navigation.sidebar, '/reference/': referenceSidebar },
    metadata.path,
  );
  const hasSidebar = sidebar.length > 0;
  const outline = metadata.outline === false ? [] : outlineHeadings(headings);
  const hasAside = metadata.aside !== false;

  return (
    <div class="flex min-h-screen flex-col">
      <NavBar metadata={metadata} />

      {/* Opens the sidebar on narrower screens, with its labels */}
      <input type="checkbox" id="sidebar-open" class="hidden" aria-hidden="true" />

      {/* The framed column holding the sidebar and the page */}
      <div
        class={classNames(
          'relative mx-auto w-full flex-1 md:max-w-[calc(100vw-2rem)] md:border-x md:border-frame min-[90rem]:max-w-[90rem]',
          hasSidebar &&
            'lg:grid lg:grid-cols-[var(--sidebar-width)_minmax(0,1fr)] lg:grid-rows-[auto_1fr]',
        )}
      >
        {/* The sidebar toggle and outline, on narrower screens */}
        <div
          class={classNames(
            'sticky top-0 z-20 w-full border-b border-frame bg-bg lg:top-[calc(var(--nav-height)-1px)] lg:col-start-2 lg:row-start-1 lg:border-b-0 xl:hidden',
            !outline.length && 'lg:hidden',
          )}
        >
          <LocalNav headings={outline} hasSidebar={hasSidebar} />
        </div>

        {hasSidebar && <SideBar groups={sidebar} path={metadata.path} />}
        {/* Clicking outside the open sidebar closes it */}
        <label
          for="sidebar-open"
          class="pointer-events-none fixed inset-0 z-50 bg-black/60 opacity-0 transition-opacity duration-500 sidebar-open:pointer-events-auto sidebar-open:opacity-100 lg:hidden"
          aria-hidden="true"
        />

        <div
          class={classNames(
            'mx-auto w-full shrink-0 grow lg:pt-(--nav-height)',
            hasSidebar && 'lg:col-start-2 lg:row-start-2 lg:m-0 lg:min-w-0',
          )}
        >
          <div class="w-full px-6 pt-8 pb-24 md:px-8 md:pt-12 md:pb-32 lg:px-8 lg:py-0">
            <div
              class={classNames(
                'mx-auto w-full xl:flex xl:justify-center',
                !hasSidebar &&
                  'lg:flex lg:max-w-[992px] lg:justify-center min-[1440px]:max-w-[1104px]',
              )}
            >
              {hasAside && (
                <div class="relative order-2 hidden w-full max-w-64 grow pl-8 xl:block">
                  <div class="pointer-events-none absolute bottom-0 left-0 z-10 h-8 w-full bg-[linear-gradient(transparent,var(--color-bg)_70%)]" />
                  <div class="sticky top-[calc(var(--nav-height)-1px)] h-[calc(100vh-var(--nav-height)+1px)] w-56 overflow-x-hidden overflow-y-auto border-l border-divider pt-5 pl-8 [scrollbar-width:none]">
                    {outline.length > 0 && <Outline headings={outline} />}
                  </div>
                </div>
              )}

              <div
                class={classNames(
                  'relative mx-auto w-full lg:px-8 lg:pt-12 lg:pb-32 xl:order-1 xl:m-0 xl:min-w-[640px]',
                  !hasSidebar && 'lg:max-w-[752px] min-[1440px]:max-w-[784px]',
                )}
              >
                <div class={classNames('mx-auto', hasAside && 'max-w-[688px]')}>
                  <main>
                    <div class="markdown">{children}</div>
                  </main>
                  <DocFooter metadata={metadata} sidebar={sidebar} />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
