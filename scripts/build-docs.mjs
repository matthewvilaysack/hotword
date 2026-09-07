// Renders the markdown docs into site/docs/ at build time, in the site's own
// style, so the docs page can never drift from the files in the repo.
// Run: bun scripts/build-docs.mjs
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { marked } from "marked";

const PAGES = [
  { slug: "index", file: "README.md", title: "Docs" },
  { slug: "design", file: "docs/design.md", title: "Design" },
  { slug: "releasing", file: "docs/releasing.md", title: "Releasing" },
  { slug: "changelog", file: "CHANGELOG.md", title: "Changelog" },
  { slug: "agents", file: "AGENTS.md", title: "Agent brief" },
];

const css = await readFile("site/docs.css", "utf8");
await mkdir("site/docs", { recursive: true });

for (const page of PAGES) {
  let md = await readFile(page.file, "utf8");
  md = md.replace(/\]\(site\//g, "](/").replace(/\]\(docs\/([\w-]+)\.md\)/g, "](/docs/$1)");
  const body = marked.parse(md, { gfm: true });
  const nav = PAGES.map((p) => `<a href="/docs${p.slug === "index" ? "" : "/" + p.slug}"${p.slug === page.slug ? ' aria-current="page"' : ""}>${p.title}</a>`).join("");
  const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${page.title === "Docs" ? "hotword docs" : `${page.title} | hotword docs`}</title>
<link href="https://fonts.googleapis.com/css2?family=Big+Shoulders+Display:wght@700;900&family=IBM+Plex+Mono:ital,wght@0,400;0,500;1,400&display=swap" rel="stylesheet">
<style>${css}</style>
</head>
<body>
<div class="wrap">
  <header class="top"><a class="home" href="/">hotword</a><nav>${nav}</nav></header>
  <main class="doc">${body}</main>
  <footer>Generated from the repository's markdown on every deploy.</footer>
</div>
</body>
</html>
`;
  await writeFile(`site/docs/${page.slug}.html`, html);
}
console.log(`site/docs: ${PAGES.length} pages`);
