import { afterEach, expect, test } from "bun:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const workflow = object(Bun.YAML.parse(readFileSync(join(root, ".github/workflows/publish.yml"), "utf8")));
const jobs = object(workflow.jobs);
const validator = object(jobs.validate);
const publisher = object(jobs.publish);
const releaseTag = "pinapod/v0.4.3";
const temporaryDirectories: string[] = [];

/** Validate objects before accessing values from the workflow YAML. */
function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/** Check YAML boundaries so workflow edits fail with a useful assertion. */
function object(value: unknown): Record<string, unknown> {
  assert(isObject(value));

  return value;
}

/** Extract the actual workflow step; regressions execute the shipped shell. */
function step(job: Record<string, unknown>, name: string): Record<string, unknown> {
  assert(Array.isArray(job.steps));
  const result: unknown = job.steps.find((value: unknown) => object(value).name === name);

  return object(result);
}

/** Run real git operations without changing the developer's checkout or identity. */
function git(cwd: string, ...args: string[]): string {
  const result = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);

  return result.stdout.trim();
}

/** Reuse real committed release records rather than mocking MonoChange output. */
function fixture(): { cwd: string; commit: string } {
  const cwd = mkdtempSync(join(tmpdir(), "publish-provenance-"));
  temporaryDirectories.push(cwd);
  git(root, "clone", "--shared", root, cwd);

  return { cwd, commit: git(cwd, "rev-parse", `refs/tags/${releaseTag}^{commit}`) };
}

/** Execute one validation step with inputs supplied through environment variables. */
function runStep(cwd: string, name: string, tag: string, commit = "") {
  const script = step(validator, name).run;
  assert(typeof script === "string");
  const output = join(cwd, "github-output");
  writeFileSync(output, "");
  const result = spawnSync("bash", ["-e", "-c", script], {
    cwd,
    encoding: "utf8",
    env: {
      ...process.env,
      RELEASE_TAG: tag,
      RELEASE_COMMIT: commit,
      RUNNER_TEMP: cwd,
      GITHUB_OUTPUT: output,
    },
  });

  return { ...result, output: readFileSync(output, "utf8") };
}

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("only dispatches on main enter the read-only validator before publication authority", () => {
  expect(Object.keys(object(workflow.on))).toEqual(["workflow_dispatch"]);
  expect(validator.if).toBe(
    "github.repository_owner == 'pina-rs' && github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/main'",
  );
  expect(validator.permissions).toEqual({ contents: "read" });
  expect(validator.environment).toBeUndefined();
  expect(step(validator, "checkout trusted main commit").with).toMatchObject({
    ref: "${{ github.sha }}",
    "fetch-depth": 0,
    "persist-credentials": false,
  });
  expect(publisher.needs).toBe("validate");
  expect(publisher.environment).toBe("publisher");
  expect(object(validator.outputs).commit).toBe("${{ steps.provenance.outputs.commit }}");
  expect(object(step(validator, "verify release record").env).RELEASE_COMMIT).toBe("${{ steps.tag.outputs.commit }}");
  expect(step(publisher, "checkout verified release commit").with).toMatchObject({
    ref: "${{ needs.validate.outputs.commit }}",
    "persist-credentials": false,
  });
});

test("a real release tag passes both gates and emits its immutable commit", () => {
  const { cwd, commit } = fixture();
  const resolved = runStep(cwd, "resolve release tag", releaseTag);
  expect(resolved.status).toBe(0);
  expect(resolved.output).toBe(`commit=${commit}\n`);
  const verified = runStep(cwd, "verify release record", releaseTag, commit);
  assert.equal(verified.status, 0, verified.stderr);
  expect(verified.output).toBe(`commit=${commit}\n`);
});

test("an annotated release tag resolves to its commit", () => {
  const { cwd, commit } = fixture();
  const result = spawnSync("git", ["mktag"], {
    cwd,
    encoding: "utf8",
    input:
      `object ${commit}\ntype commit\ntag ${releaseTag}\ntagger Release fixture <release@example.invalid> 0 +0000\n\nRelease fixture\n`,
  });
  assert.equal(result.status, 0, result.stderr);
  git(cwd, "update-ref", `refs/tags/${releaseTag}`, result.stdout.trim());
  const resolved = runStep(cwd, "resolve release tag", releaseTag);
  expect(resolved.status).toBe(0);
  expect(resolved.output).toBe(`commit=${commit}\n`);
});

test("a branch with a release-like name cannot substitute for a missing tag", () => {
  const { cwd } = fixture();
  git(cwd, "update-ref", "refs/heads/pinapod/v-test", "HEAD");
  const result = runStep(cwd, "resolve release tag", "pinapod/v-test");
  expect(result.status).not.toBe(0);
  expect(result.output).toBe("");
});

test("a branch shadowing a valid tag cannot choose the release commit", () => {
  const { cwd, commit } = fixture();
  git(cwd, "update-ref", `refs/heads/${releaseTag}`, "HEAD");
  const result = runStep(cwd, "resolve release tag", releaseTag);
  expect(result.status).toBe(0);
  expect(result.output).toBe(`commit=${commit}\n`);
});

test("a tag outside the trusted main history is rejected", () => {
  const { cwd, commit } = fixture();
  git(cwd, "checkout", "--detach", `${commit}~1`);
  const result = runStep(cwd, "resolve release tag", releaseTag);
  expect(result.status).not.toBe(0);
  expect(result.output).toBe("");
});

test("an arbitrary tag on a release commit is not authorized by its record", () => {
  const { cwd, commit } = fixture();
  git(cwd, "update-ref", "refs/tags/pinapod/v-forged", commit);
  expect(runStep(cwd, "resolve release tag", "pinapod/v-forged").status).toBe(0);
  const result = runStep(cwd, "verify release record", "pinapod/v-forged", commit);
  expect(result.status).not.toBe(0);
  expect(result.output).toBe("");
});

test("a descendant cannot inherit an older release record's authority", () => {
  const { cwd } = fixture();
  const descendant = git(cwd, "rev-parse", "HEAD");
  git(cwd, "update-ref", `refs/tags/${releaseTag}`, descendant);
  expect(runStep(cwd, "resolve release tag", releaseTag).status).toBe(0);
  const result = runStep(cwd, "verify release record", releaseTag, descendant);
  expect(result.status).not.toBe(0);
  expect(result.output).toBe("");
});

test("a tag on a commit with no release record is rejected", () => {
  const { cwd } = fixture();
  const initial = git(cwd, "rev-list", "--max-parents=0", "HEAD");
  const result = runStep(cwd, "verify release record", releaseTag, initial);
  expect(result.status).not.toBe(0);
  expect(result.output).toBe("");
});

test("moving the tag after resolution cannot change the verified checkout SHA", () => {
  const { cwd, commit } = fixture();
  expect(runStep(cwd, "resolve release tag", releaseTag).status).toBe(0);
  git(cwd, "update-ref", `refs/tags/${releaseTag}`, "HEAD");
  const result = runStep(cwd, "verify release record", releaseTag, commit);
  expect(result.status).toBe(0);
  expect(result.output).toBe(`commit=${commit}\n`);
});

test.each(["other/v1", "pinapod/v0.4.3^{commit}", "pinapod/v*", "pinapod/vbad\ncommit=forged"])(
  "malformed input %j cannot emit workflow outputs",
  (tag) => {
    const { cwd } = fixture();
    const result = runStep(cwd, "resolve release tag", tag);
    expect(result.status).not.toBe(0);
    expect(result.output).toBe("");
  },
);
