<script lang="ts">
import { CircleQuestionMark, ExternalLink, Loader, Pause, Play, Plus, Power, RefreshCw, SquarePen, Trash } from "@lucide/svelte";
import { authenticatedFetch } from "#lib/auth.js";
import { updateContainer } from "#lib/container.ts";
import { user } from "#lib/state.svelte.js";
import { formatTime } from "#lib/util.ts";

let { data } = $props();
let selectedEnvironmentId = $state();
let selectedEnvironment: any = $derived(data.development_environments.find((environment: any) => environment.id === selectedEnvironmentId));
let dialog: any = $state();
let creationDialog: any = $state();
let newName = $state("");
let newRepo = $state("");
let newBranch = $state("");
let newNestedContainers = $state(false);
let newCryptoBackend = $state("tpm");
let rotationBackend = $state("tpm");

$effect(() => {
	if (selectedEnvironmentId && !selectedEnvironment) {
		dialog?.close();
		selectedEnvironmentId = undefined;
	}
});

async function rotateSSHKey() {
	await authenticatedFetch(`/api/development_environments/${selectedEnvironment.id}/rotate_ssh_key`, {
		method: "POST",
		body: JSON.stringify({ ssh_key_backend: rotationBackend }),
	});
}

async function createEnvironment() {
	await authenticatedFetch(`/api/development_environments`, {
		method: "POST",
		body: JSON.stringify({
			name: newName,
			repo: newRepo,
			branch: newBranch,
			nested_containers: newNestedContainers,
			ssh_key_backend: newCryptoBackend,
		}),
	});
	newName = "";
	newRepo = "";
	newBranch = "";
	newNestedContainers = false;
}
async function setEnvironmentState(id: string, state: string) {
	await authenticatedFetch(`/api/development_environments/${id}/state?state=${state}`, {
		method: "POST",
	});
}
async function openDevelopmentEnvironment(id: string) {
	globalThis.open(`/embed/open_container/${id}?type=development_environment`, "_blank", "noopener");
}
</script>

<table class="border border-base-content/10 table">
	<thead>
		<tr>
			<th>State</th>
			<th>Name</th>
			<th>Repo</th>
			<th>Branch</th>
			<th>Created At</th>
			<th>Actions</th>
		</tr>
	</thead>
	<tbody>
		{#each data.development_environments as environment}
			<tr
				class="cursor-pointer hover:bg-base-200"
				onclick={() => {
					selectedEnvironmentId = environment.id;
					rotationBackend = environment.ssh_key_backend === "TPM" ? "tpm" : "software";
					dialog.showModal();
				}}
			>
				<td>
					{#if environment.pending}
						<Loader />
					{:else if environment.state === "Running"}
						<Play />
					{:else if environment.state === "Started"}
						<Pause />
					{:else if environment.state === "Stopped"}
						<Power />
					{:else if environment.state === "Non-Existent"}
						<Trash />
					{:else}
						<CircleQuestionMark />
					{/if}
				</td>
				<td>{environment.name}</td>
				<td>{environment.repo}</td>
				<td>{environment.branch}</td>
				<td>{formatTime(environment.created_at)}</td>
				<td>
					{#if !environment.pending}
						<button
							type="button"
							class="btn"
							onclick={async (e) => {
								e.stopPropagation();
								await openDevelopmentEnvironment(environment.id);
							}}
						>
							<ExternalLink />
						</button>
					{/if}
					{#if !environment.pending && (environment.state === "Running" || environment.state === "Started")}
						<button
							type="button"
							class="btn"
							onclick={async (e) => {
								e.stopPropagation();
								await setEnvironmentState(environment.id, "stop");
							}}
						>
							<Power />
						</button>
					{/if}
				</td>
			</tr>
		{/each}
	</tbody>
</table>
<div class="fab">
	<button
		type="button"
		class="btn btn-circle btn-xl btn-accent"
		onclick={() => {
			creationDialog.showModal();
		}}
	>
		<Plus />
	</button>
</div>
<dialog bind:this={dialog} class="modal">
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	{#if selectedEnvironment}
		<div class="modal-box max-w-400">
			<h2 class="text-xl text-center">Environment</h2>
			<table class="table">
				<tbody>
					<tr>
						<td>ID</td>
						<td>{selectedEnvironment.id}</td>
					</tr>
					<tr>
						<td>Name</td>
						<td>{selectedEnvironment.name}</td>
					</tr>
					<tr>
						<td>Repo</td>
						<td>{selectedEnvironment.repo}</td>
					</tr>
					<tr>
						<td>Branch</td>
						<td>{selectedEnvironment.branch}</td>
					</tr>
					<tr>
						<td>Created At</td>
						<td>{formatTime(selectedEnvironment.created_at)}</td>
					</tr>
					<tr>
						<td>State</td>
						<td>{selectedEnvironment.state}</td>
					</tr>
					<tr>
						<td>Pending</td>
						<td>{selectedEnvironment.pending}</td>
					</tr>
					<tr>
						<td>Nested Containers</td>
						<td>{selectedEnvironment.nested_containers}</td>
					</tr>
					<tr>
						<td>SSH Public Key</td>
						<td>{selectedEnvironment.ssh_key} </td>
					</tr>
					<tr>
						<td>SSH Crypto Backend</td>
						<td>{selectedEnvironment.ssh_key_backend} </td>
					</tr>
					<tr>
						<td>Rotate SSH Key</td>
						<td>
							<select class="select" bind:value={rotationBackend}>
								<option value="software">Software</option>
								<option value="tpm">TPM</option>
							</select>
							<button type="button" class="btn" onclick={async () => await rotateSSHKey()}>
								<RefreshCw />
							</button>
						</td>
					</tr>
					<tr>
						<td>Memory Limit</td>
						<td>
							{selectedEnvironment.memory_limit}
							<button
								type="button"
								class="btn"
								onclick={async () => {
									await updateContainer(selectedEnvironment.id, parseInt(prompt("Enter requested memory size in GiB") || "", 10), 0);
									dialog.close();
								}}
							>
								<SquarePen />
							</button>
						</td>
					</tr>
					<tr>
						<td>Storage Size</td>
						<td>
							{selectedEnvironment.storage_size}
							<button
								type="button"
								class="btn"
								onclick={async () => {
									await updateContainer(selectedEnvironment.id, 0, parseInt(prompt("Enter requested storage size in GiB") || "", 10));
									dialog.close();
								}}
							>
								<SquarePen />
							</button>
						</td>
					</tr>
				</tbody>
			</table>
			<div class="divider"></div>
			<form method="dialog">
				<button
					type="submit"
					class="btn"
					onclick={async () => {
						await setEnvironmentState(selectedEnvironment.id, "delete");
					}}
				>
					<Trash />
				</button>
			</form>
		</div>
	{/if}
</dialog>
<dialog bind:this={creationDialog} class="modal">
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	<div class="modal-box max-w-400">
		<h2 class="text-xl text-center">Create Environment</h2>
		<br>
		<form
			method="dialog"
			onsubmit={async () => {
				await createEnvironment();
			}}
		>
			<fieldset class="fieldset">
				<input id="environment_name" type="text" placeholder="Name" class="input" required bind:value={newName}>
				<input id="repo" type="text" placeholder="Repository" class="input" bind:value={newRepo}>
				<input id="branch" type="text" placeholder="Branch" class="input" bind:value={newBranch}>
				{#if user.admin}
					<p>
						Nested Containers:
						<input id="nested_containers" type="checkbox" class="checkbox" bind:checked={newNestedContainers}>
					</p>
				{/if}
				<p>
					SSH Crypto Backend:
					<select id="ssh_key_backend" class="select" bind:value={newCryptoBackend}>
						<option value="software">Software</option>
						<option value="tpm">TPM</option>
					</select>
				</p>
				<button type="submit" class="btn btn-primary">Create</button>
			</fieldset>
		</form>
	</div>
</dialog>
