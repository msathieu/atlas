import { toast } from "svelte-sonner";
import { authenticatedFetch } from "./auth.ts";
import { encryptToServer } from "./util.ts";

export async function updateSetting(name: string, value: string) {
  const response = await authenticatedFetch(
    `/api/settings/${name}`,
    {
      method: "PUT",
      body: await encryptToServer(JSON.stringify({ value })),
    },
    undefined,
    "application/octet-stream",
  );
  if (response.ok) {
    toast.success("Updated Setting");
  }
}
