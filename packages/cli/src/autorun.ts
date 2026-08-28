import { promises as fs } from "node:fs";
import path from "node:path";
import { exec } from "node:child_process";
import { promisify } from "node:util";

const execAsync = promisify(exec);

const AUTORUN_ICON_REL = path.join(".cat", "icon.ico");

export async function writeAutorun(driveRoot: string, volumeLabel: string): Promise<void> {
  const autorunContent = [
    "[autorun]",
    `icon=${AUTORUN_ICON_REL}`,
    `label=${volumeLabel}`,
    "",
  ].join("\r\n");

  const autorunPath = path.join(driveRoot, "autorun.inf");
  await fs.writeFile(autorunPath, autorunContent, "utf-8");

  // Windows only picks up autorun.inf reliably when it (and ideally the
  // icon) carry System+Hidden attributes — same as manufacturer USB
  // sticks/burned CDs ship theirs. Best-effort: harmless if attrib is
  // unavailable (e.g. this ever runs on macOS/Linux).
  if (process.platform === "win32") {
    try {
      await execAsync(`attrib +s +h "${autorunPath}"`);
    } catch {
      // non-fatal — icon/label are cosmetic only
    }
  }
}

export async function setVolumeLabel(driveRoot: string, volumeLabel: string): Promise<void> {
  if (process.platform !== "win32") {
    // Renaming an already-formatted volume on macOS/Linux needs
    // diskutil/e2label plus elevated permissions — out of scope for now.
    // Windows is the primary target anyway, since the launcher is a .exe.
    return;
  }
  const driveLetter = path.parse(path.resolve(driveRoot)).root.replace(/\\$/, ""); // "E:\" -> "E:"
  try {
    await execAsync(`label ${driveLetter} ${volumeLabel}`);
  } catch {
    // non-fatal
  }
}