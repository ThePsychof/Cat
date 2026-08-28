import path from "node:path";
import { fetchManifest, TargetOS } from "./manifest.js";
import { downloadAsset } from "./download.js";
import { writeAutorun, setVolumeLabel } from "./autorun.js";

const CAT_HIDDEN_DIR = ".cat";
const VOLUME_LABEL = "Cat Drive";

export async function provisionCat(
  targetPath: string,
  selectedOS: TargetOS[]
): Promise<void> {
  console.log("Fetching Cat manifest...");
  const manifest = await fetchManifest();
  console.log(`Manifest resolved: Cat v${manifest.version} (${manifest.releaseTag})`);

  const hiddenDir = path.join(targetPath, CAT_HIDDEN_DIR);

  for (const os of selectedOS) {
    const entry = manifest.assets[os];
    if (!entry) {
      console.warn(`No launcher asset defined for ${os}, skipping.`);
      continue;
    }

    console.log(`Downloading launcher for ${os}...`);
    const launcherPath = await downloadAsset(
      manifest.releaseTag,
      entry.launcher,
      targetPath // launcher goes at drive root, visible
    );
    console.log(`Launcher placed at: ${launcherPath}`);

    // Icon lives in the hidden .cat folder; autorun.inf points at it with a
    // relative path so it works regardless of which drive letter Windows
    // assigns this drive on a given machine.
    console.log(`Downloading drive icon for ${os}...`);
    await downloadAsset(manifest.releaseTag, entry.icon, hiddenDir);
  }

  // Cosmetic drive branding: custom icon + volume label. Windows still
  // parses autorun.inf for icon/label even though it ignores `open=`
  // execution (that part was disabled for security reasons around Windows 7
  // and can't be brought back).
  await writeAutorun(targetPath, VOLUME_LABEL);
  await setVolumeLabel(targetPath, VOLUME_LABEL);

  // TODO: write a small state file (.cat/state.json) recording
  // installed version + selected OS list, for the GUI's self-updater to read later.

  console.log("Cat provisioning complete.");
}