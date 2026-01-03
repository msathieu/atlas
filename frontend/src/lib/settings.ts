import { toast } from "svelte-sonner";
import { authenticatedFetch } from "./auth.ts";

export async function updateSetting(name: string, value: string) {
  const response = await authenticatedFetch(`/api/settings/${name}`, {
    method: "PUT",
    body: JSON.stringify({ value }),
  });
  if (response.ok) {
    toast.success("Updated Setting");
  }
}
