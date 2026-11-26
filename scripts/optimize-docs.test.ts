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

test("minifyCss preserves hex-escape terminators and descendant combinators", () => {
  const out = minifyCss(".\\31  .item { color: red; } .\\31 .same { color: blue; }");
  assert.equal(out, ".\\31  .item{color:red}.\\31 .same{color:blue}");
});

test("minifyJs preserves regex literals after control-flow conditions", () => {
  const js = `
    // strip comment
    if (ok) /[//]/.test(value);
    const r = /a\\/b/g;
    const d = (x + y) / 2;
  `;
  const out = minifyJs(js);
  assert.doesNotThrow(() => new vm.Script(out));
  assert.match(out, /if \(ok\) \/\[\/\/\]\/\.test\(value\);/);
  assert.match(out, /const r = \/a\\\/b\/g;/);
  assert.match(out, /const d = \(x \+ y\) \/ 2;/);
});

test("minifyHtml preserves quoted HTML attribute whitespace", () => {
  const out = minifyHtml(`<input   type="text"   value="a  b"><p>hi   there</p>`);
  assert.equal(out, `<input type="text" value="a  b"><p>hi there</p>`);
});

test("rebaseCssUrls rebases quoted URLs containing parentheses", () => {
  assert.equal(
    rebaseCssUrls(
      `background: url("icons/a(b).svg"), url('data:image/svg+xml,x');`,
      "css/general.css",
      "assets/book.css",
    ),
    `background: url("../css/icons/a(b).svg"), url('data:image/svg+xml,x');`,
  );
});

test("optimizeBook supports hash-files=false, preserves custom CSS outside theme/, and is idempotent", () => {
  const dir = mkdtempSync(join(tmpdir(), "pinapod-opt-docs-"));
  mkdirSync(join(dir, "css"));
  writeFileSync(join(dir, "css/variables.css"), ":root { --bg: #fff; }");
  writeFileSync(join(dir, "css/general.css"), "body { color: #111; }");
  writeFileSync(join(dir, "custom.css"), ".extra { color: teal; }");
  writeFileSync(join(dir, "book.js"), "// book");
  writeFileSync(join(dir, "toc.js"), "// toc");
  writeFileSync(
    join(dir, "searchindex.js"),
    `window.search = Object.assign(window.search, ${
      JSON.stringify({
        doc_urls: ["intro.html#top"],
        index: { documentStore: { docs: { "0": { breadcrumbs: "Intro", body: "amp & compact" } } } },
      })
    });`,
  );
  writeFileSync(
    join(dir, "intro.html"),
    `<!DOCTYPE html><html><head>
<link rel="stylesheet" href="css/variables.css">
<link rel="stylesheet" href="css/general.css">
<link rel="stylesheet" href="custom.css">
<script src="toc.js"></script>
</head><body><div id="mdbook-body-container"><main>Hello</main><script src="book.js"></script></div></body></html>`,
  );

  const first = optimizeBook(dir);
  assert.equal(first.htmlFilesOptimized, 1);
  assert.ok(first.bundleCssFile);
  assert.ok(first.searchJsFile);

  const html = readFileSync(join(dir, "intro.html"), "utf8");
  assert.match(html, new RegExp(first.bundleCssFile));
  assert.match(html, /<link rel="stylesheet" href="custom\.css">/);
  assert.equal(existsSync(join(dir, "custom.css")), true);

  // Second run is a no-op.
  const second = optimizeBook(dir);
  assert.equal(second.htmlFilesOptimized, 0);
  assert.equal(second.bundleCssFile, first.bundleCssFile);
  assert.equal(readFileSync(join(dir, "intro.html"), "utf8"), html);

  const compact = buildCompactSearchData(
    `window.search = { doc_urls: ["a.html"], index: { documentStore: { docs: { "0": { title: "A", body: "b" } } } } };`,
  );
  assert.deepEqual(compact, { u: ["a.html"], d: [["A", "b"]] });
});
