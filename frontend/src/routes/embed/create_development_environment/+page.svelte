<script lang="ts">
import { authenticatedFetch } from "#lib/auth.ts";
import { user } from "#lib/state.svelte.ts";

let { data } = $props();

let name = $state("");
let nestedContainers = $state(false);
let cryptoBackend = $state("tpm");

async function createEnvironment() {
	const response = await authenticatedFetch(`/api/development_environments`, {
		method: "POST",
		body: JSON.stringify({
			name: name,
			repo: data.repo,
			branch: "",
			nested_containers: nestedContainers,
			ssh_key_backend: cryptoBackend,
		}),
	});
	const json = await response.json();
	globalThis.open(`/embed/open_container/${json.id}?type=development_environment`, "_blank", "noopener");
	globalThis.close();
}
</script>

<div class="flex">
	<div class="m-auto">
		<fieldset class="fieldset text-xl">
			<p>Create Development Environment?</p>
			<p>Repo: {data.repo}</p>
			<input id="environment_name" type="text" placeholder="Name" class="input" required bind:value={name}>
			{#if user.admin}
				<p>
					Nested Containers:
					<input id="nested_containers" type="checkbox" class="checkbox" bind:checked={nestedContainers}>
				</p>
			{/if}
			<p>
				SSH Crypto Backend:
				<select id="ssh_key_backend" class="select" bind:value={cryptoBackend}>
					<option value="software">Software</option>
					<option value="tpm">TPM</option>
				</select>
			</p>
			<button
				type="button"
				class="btn btn-primary"
				onclick={async () => {
					await createEnvironment();
				}}
			>
				Create
			</button>
			<button
				type="button"
				class="btn btn-secondary"
				onclick={() => {
					globalThis.close();
				}}
			>
				Cancel
			</button>
		</fieldset>
	</div>
</div>
