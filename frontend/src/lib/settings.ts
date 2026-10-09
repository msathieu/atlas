import { toast } from "svelte-sonner";
import { authenticatedFetch } from "./auth.ts";
import { encryptToServer } from "./util.ts";

export async function updateSetting(name: string, value: string, global: boolean) {
	const body = JSON.stringify({ value });
	let response: Response;
	if (name === "development_environment_settings" && !global) {
		response = await authenticatedFetch(`/api/user/settings/${name}`, {
			method: "PUT",
			body,
		});
	} else {
		const url = global ? `/api/settings/${name}` : `/api/user/settings/${name}`;
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
