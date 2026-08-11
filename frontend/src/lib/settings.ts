import { toast } from "svelte-sonner";
import { authenticatedFetch } from "./auth.ts";
import { encryptToServer } from "./util.ts";

export async function updateSetting(name: string, value: string, global: boolean) {
  const body = JSON.stringify({ value });
  let response;
  if (name === "development_environment_settings" && !global) {
    response = await authenticatedFetch(`/api/user/settings/${name}`, { method: "PUT", body });
  } else {
    let url;
    if (global) {
      url = `/api/settings/${name}`;
    } else {
      url = `/api/user/settings/${name}`;
    }
    response = await authenticatedFetch(
      url,
      {
        method: "PUT",
        body: await encryptToServer(body),
      },
      undefined,
      "application/octet-stream",
    );
  }
  if (response.ok) {
    toast.success("Updated Setting");
  }
}
