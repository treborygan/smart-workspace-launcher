import { save } from "@tauri-apps/plugin-dialog";

export function chooseUsageExportPath() {
  return save({
    defaultPath: "workspace-usage.csv",
    filters: [{ name: "Comma-separated values", extensions: ["csv"] }],
  });
}
