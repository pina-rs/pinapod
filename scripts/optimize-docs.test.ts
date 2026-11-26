import { test } from "bun:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import vm from "node:vm";

import {
  buildCompactSearchData,
  minifyCss,
  minifyHtml,
  minifyJs,
  optimizeBook,
  rebaseCssUrls,
} from "./optimize-docs.ts";

test("minifyCss preserves hex-escape terminators, escaped punctuation, and descendant combinators", () => {
  const out = minifyCss(
    ".\\31  .item { color: red; } .\\31 .same { color: blue; } .x\\> .y { color: green; } a > b { color: black; }",
  );
  assert.equal(out, ".\\31  .item{color:red}.\\31 .same{color:blue}.x\\> .y{color:green}a>b{color:black}");
});

test("minifyJs preserves regex literals after control-flow conditions and all ECMAScript line terminators", () => {
  const js = `
    // strip comment
    if (ok) /[//]/.test(value);
    const r = /a\\/b/g;
    const d = (x + y) / 2;
    function f() { return/*\r*/ 1; }
    function g() { return/*\u2028*/ 2; }
  `;
  const out = minifyJs(js);
  assert.doesNotThrow(() => new vm.Script(out));
  assert.match(out, /if \(ok\) \/\[\/\/\]\/\.test\(value\);/);
  assert.match(out, /const r = \/a\\\/b\/g;/);
  assert.match(out, /const d = \(x \+ y\) \/ 2;/);
  const sandbox: Record<string, unknown> = { ok: true, value: "/", x: 2, y: 4 };
  vm.runInNewContext(`${out}\nglobalThis.__f = f(); globalThis.__g = g();`, sandbox);
  assert.equal(sandbox.__f, undefined);
  assert.equal(sandbox.__g, undefined);
});

test("minifyHtml preserves quoted HTML attribute whitespace including tag-like text", () => {
  const out = minifyHtml(
    `<input   type="text"   value="a  b"><p title="left>  <div right" data-plain="x  y">  hi   there  </p>`,
  );
  assert.equal(
    out,
    `<input type="text" value="a  b"><p title="left>  <div right" data-plain="x  y">hi there</p>`,
  );
});

test("rebaseCssUrls rebases quoted URLs with parentheses and uppercase URL() preserving path case", () => {
  assert.equal(
    rebaseCssUrls(
      `background: url("icons/a(b).svg"), URL("icons/Logo.svg"), url('data:image/svg+xml,x');`,
      "css/general.css",
      "assets/book.css",
    ),
    `background: url("../css/icons/a(b).svg"), url("../css/icons/Logo.svg"), url('data:image/svg+xml,x');`,
  );
});

test("buildCompactSearchData carries mdBook limit_results", () => {
  const compact = buildCompactSearchData(
    `window.search = { results_options: { limit_results: 2 }, doc_urls: ["a.html"], index: { documentStore: { docs: { "0": { title: "A", body: "b" } } } } };`,
  );
  assert.deepEqual(compact, { u: ["a.html"], d: [["A", "b"]], l: 2 });
});

test("optimizeBook supports hash-files=false, classic script search data, deferred book.js, cascade order, @import exclusion, unquoted link attrs, standalone CSS retention, and idempotency", () => {
  const dir = mkdtempSync(join(tmpdir(), "pinapod-opt-docs-"));
  mkdirSync(join(dir, "css"));
  mkdirSync(join(dir, "theme"));
  writeFileSync(join(dir, "css/variables.css"), ":root { --bg: #fff; }");
  writeFileSync(join(dir, "css/general.css"), "body { color: #111; }");
  writeFileSync(join(dir, "custom.css"), ".extra { color: teal; }");
  writeFileSync(join(dir, "theme/override.css"), ".extra { color: gold; }");
  writeFileSync(join(dir, "theme/imported.css"), `@import url("colors.css");\n.imp { color: purple; }`);
  writeFileSync(join(dir, "book.js"), "// book");
  writeFileSync(
    join(dir, "searchindex.js"),
    `window.search = Object.assign(window.search, ${
      JSON.stringify({
        results_options: { limit_results: 1 },
        doc_urls: ["intro.html#top", "second.html#top"],
        index: {
          documentStore: {
            docs: {
              "0": { breadcrumbs: "Intro", body: "amp & compact" },
              "1": { breadcrumbs: "Second", body: "compact second" },
            },
          },
        },
      })
    });`,
  );
  // Chapter uses unquoted `<link rel=stylesheet href=css/variables.css>`,
  // references `@import` theme sheet + custom.css BEFORE theme/override.css,
  // and uses `<script defer src="book.js">` without toc.js.
  writeFileSync(
    join(dir, "intro.html"),
    `<!DOCTYPE html><html><head>
<link rel=stylesheet href=css/variables.css>
<link rel="stylesheet" href="css/general.css">
<link rel="stylesheet" href="theme/imported.css">
<link rel="stylesheet" href="custom.css">
<link rel="stylesheet" href="theme/override.css">
</head><body><div id="mdbook-body-container"><main>Hello</main><script defer src="book.js"></script></div></body></html>`,
  );
  // Standalone page still links to css/general.css.
  writeFileSync(
    join(dir, "standalone.html"),
    `<!DOCTYPE html><html><head><link rel="stylesheet" href="css/general.css"></head><body><main>Standalone</main></body></html>`,
  );

  const first = optimizeBook(dir);
  assert.equal(first.htmlFilesOptimized, 2);
  assert.ok(first.bundleCssFile);
  assert.ok(first.searchJsFile);
  assert.match(first.searchDataFile ?? "", /^assets\/search-data-[a-f0-9]{8}\.js$/);

  const html = readFileSync(join(dir, "intro.html"), "utf8");
  assert.match(html, new RegExp(first.bundleCssFile));
  // Cascade order preserved: bundle -> theme/imported.css -> custom.css -> theme/override.css
  const bundleIdx = html.indexOf(first.bundleCssFile);
  const importedIdx = html.indexOf("theme/imported.css");
  const customIdx = html.indexOf("custom.css");
  const overrideIdx = html.indexOf("theme/override.css");
  assert.ok(bundleIdx >= 0 && importedIdx > bundleIdx && customIdx > importedIdx && overrideIdx > customIdx);
  assert.equal(existsSync(join(dir, "theme/imported.css")), true);
  assert.equal(existsSync(join(dir, "custom.css")), true);
  assert.equal(existsSync(join(dir, "theme/override.css")), true);

  // Deferred `<script defer src="book.js">` was replaced with the lean bundle and book.js cleaned up.
  assert.match(html, new RegExp(`defer src="${first.bundleJsFile}"`));
  assert.equal(existsSync(join(dir, "book.js")), false);

  // Standalone page still references css/general.css so it must NOT be deleted,
  // while css/variables.css (rewritten from unquoted `<link rel=stylesheet>`) IS deleted.
  assert.equal(existsSync(join(dir, "css/general.css")), true);
  assert.equal(existsSync(join(dir, "css/variables.css")), false);

  // Classic script search data populates window.__pinaSearchData and retains limit_results=1.
  const searchDataScript = readFileSync(join(dir, first.searchDataFile!), "utf8");
  const sb: Record<string, unknown> = { window: {} };
  vm.runInNewContext(searchDataScript, sb);
  assert.equal((sb.window as { __pinaSearchData?: { l?: number } }).__pinaSearchData?.l, 1);

  // Second run on the same directory (even with retained standalone css/general.css) is a no-op.
  const second = optimizeBook(dir);
  assert.equal(second.htmlFilesOptimized, 0);
  assert.equal(second.bundleCssFile, first.bundleCssFile);
  assert.equal(readFileSync(join(dir, "intro.html"), "utf8"), html);
});
