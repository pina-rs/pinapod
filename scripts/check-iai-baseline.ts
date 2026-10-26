/**
 * Instruction-count regression gate for the `iai_hot_paths` bench.
 *
 * Compares the Callgrind instruction counts (Ir) that iai-callgrind just
 * measured under `target/iai` against the committed baseline in
 * `pinapod/benches/iai-baseline.json`, and exits non-zero when any hot path
 * exceeded the tolerance. Instruction counts are a property of the binary,
 * so a fresh Linux runner produces the same numbers as the machine that
 * recorded the baseline, and any difference is a real change made by the
 * pull request.
 *
 * Usage (run after `cargo bench --bench iai_hot_paths -- --save-summary=json`):
 *
 *   deno run --allow-read --allow-write scripts/check-iai-baseline.ts
 *   deno run --allow-read --allow-write scripts/check-iai-baseline.ts --update
 *
 * `--update` rewrites the baseline from the fresh measurements. Use it only
 * for changes that are expected to move instruction counts (a toolchain pin
 * bump, an accepted hot-path change), and say so in the pull request.
 */

const TOLERANCE = 0.03;

const DEFAULT_SUMMARY_DIR = "target/iai";
const DEFAULT_BASELINE = "pinapod/benches/iai-baseline.json";

type Baseline = Record<string, number>;

/** One benchmark summary as iai-callgrind writes it under `--save-summary=json`. */
type BenchmarkSummary = {
  module_path?: string;
  id?: string | null;
  profiles?: Array<{
    summaries?: {
      parts?: Array<{
        metrics_summary?: {
          Callgrind?: Record<
            string,
            { metrics?: { Left?: { Int?: number } } }
          >;
        };
      }>;
    };
  }>;
};

function parseArgs(): {
  update: boolean;
  summaryDir: string;
  baselinePath: string;
} {
  const args = Deno.args;
  const update = args.includes("--update");
  const positional = args.filter((arg) => !arg.startsWith("--"));
  const [summaryDir = DEFAULT_SUMMARY_DIR, baselinePath = DEFAULT_BASELINE] = positional;

  return { update, summaryDir, baselinePath };
}

/** Collect every summary file iai-callgrind wrote, newest tree walk order. */
async function collectSummaryFiles(dir: string): Promise<string[]> {
  const files: string[] = [];

  try {
    for await (const entry of Deno.readDir(dir)) {
      if (entry.isFile && entry.name.endsWith(".json")) {
        files.push(`${dir}/${entry.name}`);
      } else if (entry.isDirectory) {
        files.push(...(await collectSummaryFiles(`${dir}/${entry.name}`)));
      }
    }
  } catch (error) {
    throw new Error(
      `cannot read summary directory '${dir}': run the bench with --save-summary=json first (${
        error instanceof Error ? error.message : String(error)
      })`,
    );
  }

  return files;
}

/** Summary files are one JSON object per benchmark per line. */
function parseSummary(text: string): BenchmarkSummary[] {
  const lines = text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  return lines.flatMap((line) => {
    try {
      const parsed = JSON.parse(line) as
        | BenchmarkSummary
        | BenchmarkSummary[];

      return Array.isArray(parsed) ? parsed : [parsed];
    } catch {
      return [];
    }
  });
}

function benchmarkId(summary: BenchmarkSummary): string | null {
  const module = summary.module_path ?? "";

  // The runner already nests the function under its group module
  // ("iai_hot_paths::fixed::fixed_parse"), so the module path alone is the
  // unique, stable key; a benchmark with an explicit id appends it.
  if (module === "") {
    return null;
  }

  return summary.id ? `${module}::${summary.id}` : module;
}

function instructionCount(summary: BenchmarkSummary): number | null {
  let total = 0;
  let found = false;

  for (const profile of summary.profiles ?? []) {
    for (const part of profile.summaries?.parts ?? []) {
      const count = part.metrics_summary?.Callgrind?.Ir?.metrics?.Left?.Int;

      if (typeof count === "number") {
        total += count;
        found = true;
      }
    }
  }

  return found ? total : null;
}

async function measure(summaryDir: string): Promise<Map<string, number>> {
  const measured = new Map<string, number>();

  for (const file of await collectSummaryFiles(summaryDir)) {
    for (const summary of parseSummary(await Deno.readTextFile(file))) {
      const instructions = instructionCount(summary);

      if (instructions === null) {
        continue;
      }

      const id = benchmarkId(summary);

      if (id === null) {
        continue;
      }

      measured.set(id, instructions);
    }
  }

  return measured;
}

async function readBaseline(path: string): Promise<Baseline> {
  try {
    return JSON.parse(await Deno.readTextFile(path)) as Baseline;
  } catch {
    throw new Error(
      `cannot read baseline '${path}': generate it with --update on Linux (see docs/benchmarks.md)`,
    );
  }
}

function sortRecord(record: Baseline): Baseline {
  return Object.fromEntries(
    Object.entries(record).sort(([a], [b]) => a.localeCompare(b)),
  );
}

async function main(): Promise<void> {
  const { update, summaryDir, baselinePath } = parseArgs();
  const measured = await measure(summaryDir);

  if (measured.size === 0) {
    throw new Error(
      "no benchmark instruction counts found under the summary directory",
    );
  }

  if (update) {
    const baseline = sortRecord(Object.fromEntries(measured));

    await Deno.writeTextFile(
      baselinePath,
      `${JSON.stringify(baseline, null, "\t")}\n`,
    );
    console.log(
      `baseline updated: ${baselinePath} (${measured.size} benchmarks)`,
    );

    return;
  }

  const baseline = await readBaseline(baselinePath);
  const baselineIds = new Set(Object.keys(baseline));
  const measuredIds = new Set(measured.keys());

  const missing = [...baselineIds].filter((id) => !measuredIds.has(id));
  const added = [...measuredIds].filter((id) => !baselineIds.has(id));

  if (missing.length > 0 || added.length > 0) {
    for (const id of added) {
      console.error(
        `NEW      ${id}: ${measured.get(id)} Ir (absent from the baseline)`,
      );
    }

    for (const id of missing) {
      console.error(
        `GONE     ${id}: baseline ${baseline[id]} Ir (not measured)`,
      );
    }

    throw new Error(
      "the benchmark set changed; rerun with --update and justify the baseline change in the pull request",
    );
  }

  const failures: string[] = [];

  for (const [id, expected] of Object.entries(baseline)) {
    const actual = measured.get(id) as number;
    const limit = Math.ceil(expected * (1 + TOLERANCE));
    const delta = actual - expected;

    if (actual > limit) {
      failures.push(
        `REGRESS  ${id}: ${actual} Ir vs baseline ${expected} Ir (+${delta}, tolerance +${TOLERANCE * 100}%)`,
      );
    } else if (delta !== 0) {
      const direction = delta > 0 ? "+" : "";
      console.log(`ok       ${id}: ${actual} Ir (${direction}${delta})`);
    } else {
      console.log(`ok       ${id}: ${actual} Ir`);
    }
  }

  if (failures.length > 0) {
    for (const failure of failures) {
      console.error(failure);
    }

    throw new Error(
      `${failures.length} hot path(s) regressed beyond the ${TOLERANCE * 100}% instruction tolerance`,
    );
  }

  console.log(
    `instruction gate passed: ${measured.size} benchmarks within +${TOLERANCE * 100}% of the baseline`,
  );
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? `error: ${error.message}` : error);
  Deno.exit(1);
}
