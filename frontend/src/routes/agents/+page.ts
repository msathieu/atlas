import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
  const agents = await authenticatedFetch("/api/agents", {}, fetch);
  return { agents: await agents.json() };
};
