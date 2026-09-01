#!/usr/bin/env node
import { Command } from "commander";
import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { provisionCat } from "./provision.js";
import type { TargetOS } from "./manifest.js";
import { CatSpinner } from "./spinner.js";

const ALL_OS: TargetOS[] = ["windows", "macos", "linux"];

async function promptForOS(): Promise<TargetOS[]> {
  const rl = readline.createInterface({ input, output });
  try {
    const answer = await rl.question(
      "Which OS(es) should this Cat support?\n" +
      "  [1] Windows\n" +
      "  [2] macOS\n" +
      "  [3] Linux\n" +
      "  [4] All\n" +
      "Choice (default: 1): "
    );

    const trimmed = answer.trim();
    if (trimmed === "" || trimmed === "1") return ["windows"];
    if (trimmed === "2") return ["macos"];
    if (trimmed === "3") return ["linux"];
    if (trimmed === "4") return ALL_OS;

    console.log("Unrecognized choice, defaulting to Windows.");
    return ["windows"];
  } finally {
    rl.close();
  }
}

async function initCat(
  targetPath: string,
  localAssetsRoot?: string,
  manifestUrlOverride?: string
): Promise<void> {
  if (localAssetsRoot) {
    process.env.CAT_LOCAL_ASSET_ROOT = localAssetsRoot;
  }
  if (manifestUrlOverride) {
    process.env.CAT_MANIFEST_URL = manifestUrlOverride;
  }

  const selectedOS = await promptForOS(); // readline prompt runs before the spinner takes the line over

  const spinner = new CatSpinner();
  spinner.start(`Initializing Cat at: ${targetPath}`);
  try {
    await provisionCat(targetPath, selectedOS, spinner, manifestUrlOverride);
    spinner.succeed(`Cat is ready at ${targetPath}!`);
  } catch (err) {
    spinner.fail(`Something went wrong: ${(err as Error).message}`);
    process.exitCode = 1;
  }
}

const program = new Command();

program
  .name("mewmew")
  .description("Provision a portable drive into a Cat environment");

program
  .command("init")
  .description("Turn a drive into a Cat")
  .argument("[path]", "path to the target drive", ".")
  .option("--local-assets <path>", "use a local fallback directory for manifest and launcher assets")
  .option("--manifest-url <url>", "override the manifest source for local/offline testing")
  .action(async (path: string, options: { localAssets?: string; manifestUrl?: string }) => {
    await initCat(path, options.localAssets, options.manifestUrl);
  });

program.parseAsync(process.argv);