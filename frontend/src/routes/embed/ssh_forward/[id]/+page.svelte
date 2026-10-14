<script lang="ts">
import { encryptedFetch } from "#lib/util.ts";

let { data } = $props();

setTimeout(() => {
	globalThis.close();
}, 60000);

async function approveRequest(approved: boolean) {
	await encryptedFetch(`/api/development_environments/ssh_forward/approve/${data.id}`, {
		method: "POST",
		body: JSON.stringify({ approved }),
	});
	globalThis.close();
}
</script>

<div class="flex">
	<div class="m-auto">
		<fieldset class="fieldset text-xl">
			<p>Approve SSH signing request?</p>
			<p>ID: {data.development_environment_id}</p>
			<p>Name: {data.name}</p>
			<button
				type="button"
				class="btn btn-primary"
				onclick={async () => {
					await approveRequest(true);
				}}
			>
				Approve
			</button>
			<button
				type="button"
				class="btn btn-secondary"
				onclick={async () => {
					await approveRequest(false);
				}}
			>
				Reject
			</button>
		</fieldset>
	</div>
</div>
