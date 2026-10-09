import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
	const development_environments = await authenticatedFetch("/api/development_environments", {}, fetch);
	return { development_environments: await development_environments.json() };
};
