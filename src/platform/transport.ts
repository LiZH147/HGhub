/**
 * Host transport: the Tauri bridge in the desktop build, same-origin HTTP in the Web build.
 * Plugin calls keep one shape in both cases, so pages do not branch on the container.
 */
const TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Desktop/mobile app build; false when the same UI runs in a browser. */
export const isTauriHost = TAURI;

type HostArgs = Record<string, unknown> | undefined;

async function webInvoke<T>(command: string, args?: HostArgs): Promise<T> {
  if (command === "hpp_manage") {
    const response = await fetch("/rpc", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        action: (args as { action: string }).action,
        params: (args as { params?: unknown }).params ?? {},
      }),
    });
    const body = (await response.json()) as { ok: boolean; value?: T; error?: { code: string; message: string } };
    if (!body.ok) throw body.error;
    return body.value as T;
  }
  if (command === "app_boot") return { strict_security: true } as T;
  if (command === "debug_log") {
    console.debug((args as { msg?: string })?.msg ?? "");
    return undefined as T;
  }
  if (command === "open_url" || command === "open_external_media") {
    window.open(String((args as { url: string }).url), "_blank", "noopener");
    return undefined as T;
  }
  throw new Error(`Web 版暂不支持命令 ${command}`);
}

export async function hostInvoke<T>(command: string, args?: HostArgs): Promise<T> {
  if (TAURI) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(command, args);
  }
  return webInvoke<T>(command, args);
}
