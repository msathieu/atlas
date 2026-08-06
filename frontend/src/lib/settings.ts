import { toast } from "svelte-sonner";
import { authenticatedFetch } from "./auth.ts";
import { encryptToServer } from "./util.ts";

export async function updateSetting(name: string, value: string) {
  const body = JSON.stringify({ value });
  let response;
  if (name === "development_environment_settings") {
    response = await authenticatedFetch(`/api/settings/${name}`, { method: "PUT", body });
  } else {
    response = await authenticatedFetch(
      `/api/settings/${name}`,
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
