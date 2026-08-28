#!/usr/bin/env node
import { Command } from "commander";
import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { provisionCat } from "./provision.js";
import type { TargetOS } from "./manifest.js";

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

async function initCat(targetPath: string): Promise<void> {
  console.log(`Initializing Cat at: ${targetPath}`);
  const selectedOS = await promptForOS();
  console.log(`Selected OS target(s): ${selectedOS.join(", ")}`);

  await provisionCat(targetPath, selectedOS);
}

const program = new Command();

program
  .name("miuu")
  .description("Provision a portable drive into a Cat environment");

program
  .command("init")
  .description("Turn a drive into a Cat")
  .argument("[path]", "path to the target drive", ".")
  .action(async (path: string) => {
    await initCat(path);
  });

program.parseAsync(process.argv);