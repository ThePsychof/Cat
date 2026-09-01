import path from "node:path";
import { fetchManifest, TargetOS } from "./manifest.js";
import { downloadAsset } from "./download.js";
import { writeAutorun, setVolumeLabel } from "./autorun.js";
import type { CatSpinner } from "./spinner.js";
import { writeState } from "./state.js";

const CAT_HIDDEN_DIR = ".cat";
const VOLUME_LABEL = "Cat";

export async function provisionCat(
  targetPath: string,
  selectedOS: TargetOS[],
  spinner?: CatSpinner,
  manifestUrlOverride?: string,
  assetBaseUrlOverride?: string
): Promise<void> {
  const say = (text: string) => (spinner ? spinner.update(text) : console.log(text));

  say("Fetching Cat manifest...");
  const manifest = await fetchManifest(manifestUrlOverride);
  say(`Manifest resolved: Cat v${manifest.version} (${manifest.releaseTag})`);

  const hiddenDir = path.join(targetPath, CAT_HIDDEN_DIR);

  for (const os of selectedOS) {
    const entry = manifest.assets[os];
    if (!entry) {
      say(`No launcher asset defined for ${os}, skipping.`);
      continue;
    }

    say(`Downloading launcher for ${os}...`);
    const launcherPath = await downloadAsset(
      manifest.releaseTag,
      entry.launcher,
      targetPath,
      entry.launcherSha256,
      assetBaseUrlOverride
    );
    say(`Launcher placed at: ${launcherPath}`);

    say(`Downloading drive icon for ${os}...`);
    await downloadAsset(
      manifest.releaseTag,
      entry.icon,
      hiddenDir,
      entry.iconSha256,
      assetBaseUrlOverride
    );
  }

  say("Branding the drive...");
  await writeAutorun(targetPath, VOLUME_LABEL);
  await setVolumeLabel(targetPath, VOLUME_LABEL);

  say("Writing initial state...");
  await writeState(targetPath, {
    version: manifest.version,
    selectedOS,
    activeProfile: null,
    profiles: [],
    repos: [],
  });

  say("Cat provisioning complete.");
}