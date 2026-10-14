<script lang="ts">
import { user } from "#lib/state.svelte.ts";
import { encryptedFetch } from "#lib/util.ts";

let { data } = $props();

let name = $state("");
let nestedContainers = $state(false);
let cryptoBackend = $state("tpm");

async function createEnvironment() {
	const response = await encryptedFetch(`/api/development_environments`, {
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
			<div>
				<label for="environment_name">Name</label>
				<input id="environment_name" type="text" placeholder="Name" class="input" required bind:value={name}>
			</div>
			{#if user.admin}
				<div>
					<label for="nested_containers">Nested Containers</label>
					<input id="nested_containers" type="checkbox" class="checkbox" bind:checked={nestedContainers}>
				</div>
			{/if}
			<div>
				<label for="ssh_key_backend">SSH Crypto Backend</label>
				<select id="ssh_key_backend" class="select" bind:value={cryptoBackend}>
					<option value="software">Software</option>
					<option value="tpm">TPM</option>
				</select>
			</div>
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
