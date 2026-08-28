import type { CatManifest } from "./manifest.js";

// Temporary local stand-in for fetchManifest() until real releases exist
// on ThePsychof/Cat. Swap the import in provision.ts back to "./manifest.js"
// once a real manifest.json + tagged release is published.
export function getLocalManifest(): CatManifest {
  return {
    version: "0.0.1-dev",
    releaseTag: "v0.0.1-dev",
    assets: {
      windows: { launcher: "cat-launcher-windows.exe" },
      macos: { launcher: "cat-launcher-macos.app.zip" },
      linux: { launcher: "cat-launcher-linux" },
    },
  };
}