import { toast } from "svelte-sonner";
import { encryptedFetch } from "./util.ts";

export async function updateSetting(name: string, value: string, global: boolean) {
	const body = JSON.stringify({ value });
	const url = global ? `/api/settings/${name}` : `/api/user/settings/${name}`;
	const response = await encryptedFetch(url, {
		method: "PUT",
		body,
	});
	if (response.ok) {
		toast.success("Updated Setting");
	}
}
