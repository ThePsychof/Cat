import path from "node:path";
import { fetchManifest, TargetOS } from "./manifest.js";
import { downloadAsset } from "./download.js";

const CAT_HIDDEN_DIR = ".cat";

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
  }

  // TODO: write a small state file (.cat/state.json) recording
  // installed version + selected OS list, for the GUI's self-updater to read later.

  console.log("Cat provisioning complete.");
  void hiddenDir; // reserved for future use (credentials/state live here, written by the GUI)
}