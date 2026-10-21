import { open, save } from "@tauri-apps/plugin-dialog";

export async function chooseProfilePackage(): Promise<string | null> {
  const result = await open({ multiple: false, directory: false, filters: [{ name: "Workspace profile package", extensions: ["swprofile"] }] });
  return typeof result === "string" ? result : null;
}

export async function chooseProfileExportPath(profileName: string): Promise<string | null> {
  return save({ defaultPath: `${safeFilename(profileName)}.swprofile`, filters: [{ name: "Workspace profile package", extensions: ["swprofile"] }] });
}

function safeFilename(value: string) { return value.replace(/[\\/:*?"<>|]/g, "-").trim() || "workspace-profile"; }
