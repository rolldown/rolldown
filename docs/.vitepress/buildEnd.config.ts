import path from 'node:path';
import { writeFileSync } from 'node:fs';
import { Feed } from 'feed';
import type { SiteConfig } from 'vitepress';
import { createContentLoader } from 'vitepress';

const siteUrl = 'https://rolldown.rs';
const blogUrl = `${siteUrl}/blog`;

export const buildEnd = async (config: SiteConfig): Promise<void> => {
  const feed = new Feed({
    title: 'Rolldown',
    description: 'Fast Rust-based bundler for JavaScript with Rollup-compatible API',
    id: blogUrl,
    link: blogUrl,
    language: 'en',
    image: 'https://rolldown.rs/og.jpg',
    favicon: 'https://rolldown.rs/logo-without-border.svg',
    copyright: 'Copyright © 2024-present VoidZero Inc. and Rolldown Contributors',
  });

  const posts = await createContentLoader('blog/*.md', {
    excerpt: true,
    render: true,
  }).load();

  posts.sort(
    (a, b) => +new Date(b.frontmatter.date as string) - +new Date(a.frontmatter.date as string),
  );

  for (const { url, excerpt, frontmatter, html } of posts) {
    feed.addItem({
      title: frontmatter.title,
      id: `${siteUrl}${url}`,
      link: `${siteUrl}${url}`,
      description: excerpt,
      content: html,
      author: [
        {
          name: frontmatter.author.name,
        },
      ],
      date: frontmatter.date,
    });
  }

  writeFileSync(path.join(config.outDir, 'blog.rss'), feed.rss2());
};
