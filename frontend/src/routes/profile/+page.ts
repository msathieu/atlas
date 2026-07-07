import { authenticatedFetch } from "$lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async () => {
  const passkeys = await authenticatedFetch("/api/passkeys", {}, fetch);
  return { passkeys: await passkeys.json() };
};
