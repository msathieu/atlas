import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
	const users = await authenticatedFetch("/api/users", {}, fetch);
	return { users: await users.json() };
};
