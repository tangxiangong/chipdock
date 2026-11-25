import { cleanup, Storage } from "nsh";
import { checkForPackage, downloadPackage, shellInfo } from "nsh/updater";

export function startInterfaceUpdates() {
  if (!shellInfo().packages) return;
  let checking = false;
  const stagedBuildKey = "ota.lastStagedBuild";
  async function refresh() {
    if (checking) return;
    checking = true;
    try {
      const available = await checkForPackage();
      if (available && available.build > Number(await Storage.getItem(stagedBuildKey))) {
        await downloadPackage();
        await Storage.setItem(stagedBuildKey, String(available.build));
      }
    } catch (error) {
      console.warn(`ChipDock OTA: ${String(error)}`);
    } finally {
      checking = false;
    }
  }
  void refresh();
  const timer = setInterval(refresh, 24 * 60 * 60 * 1000);
  cleanup(() => clearInterval(timer));
}
