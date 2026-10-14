<script lang="ts">
import { CircleQuestionMark, ExternalLink, Loader, Pause, Play, Plus, Power, SquarePen, Trash } from "@lucide/svelte";
import { authenticatedFetch } from "#lib/auth.ts";
import { updateContainer } from "#lib/container.ts";
import { encryptedFetch, formatTime } from "#lib/util.ts";

let { data } = $props();
let selectedAgentId = $state();
let selectedAgent: any = $derived(data.agents.find((agent: any) => agent.id === selectedAgentId));
let dialog: any = $state();
let creationDialog: any = $state();
let newName = $state("");
let forgeToken = $state("");
let forgeWebhookSecret = $state("");

$effect(() => {
	if (selectedAgentId && !selectedAgent) {
		dialog?.close();
		selectedAgentId = undefined;
	}
});

function resetForgeInputs() {
	forgeToken = "";
	forgeWebhookSecret = "";
}
async function updateAgentForge(name: string, value: string) {
	await encryptedFetch(`/api/agents/${selectedAgent.id}/forge`, {
		method: "PATCH",
		body: JSON.stringify({ [name]: value }),
	});
}
async function createAgent() {
	await authenticatedFetch(`/api/agents`, {
		method: "POST",
		body: JSON.stringify({ name: newName }),
	});
}
async function setAgentState(id: string, state: string) {
	await authenticatedFetch(`/api/agents/${id}/state?state=${state}`, {
		method: "POST",
	});
}
async function openAgent(id: string) {
	globalThis.open(`/embed/open_container/${id}?type=agent`, "_blank", "noopener");
}
</script>

<table class="border border-base-content/10 table">
	<thead>
		<tr>
			<th>State</th>
			<th>Name</th>
			<th>Created At</th>
			<th>Actions</th>
		</tr>
	</thead>
	<tbody>
		{#each data.agents as agent}
			<tr
				class="cursor-pointer hover:bg-base-200"
				onclick={() => {
					selectedAgentId = agent.id;
					dialog.showModal();
				}}
			>
				<td>
					{#if agent.pending}
						<Loader />
					{:else if agent.state === "Running"}
						<Play />
					{:else if agent.state === "Started"}
						<Pause />
					{:else if agent.state === "Stopped"}
						<Power />
					{:else if agent.state === "Non-Existent"}
						<Trash />
					{:else}
						<CircleQuestionMark />
					{/if}
				</td>
				<td>{agent.name}</td>
				<td>{formatTime(agent.created_at)}</td>
				<td>
					{#if !agent.pending}
						<button
							type="button"
							class="btn"
							onclick={async (e) => {
								e.stopPropagation();
								await openAgent(agent.id);
							}}
						>
							<ExternalLink />
						</button>
					{/if}
					{#if !agent.pending && (agent.state === "Running" || agent.state === "Started")}
						<button
							type="button"
							class="btn"
							onclick={async (e) => {
								e.stopPropagation();
								await setAgentState(agent.id, "stop");
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
<dialog bind:this={dialog} class="modal" onclose={resetForgeInputs}>
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	{#if selectedAgent}
		<div class="modal-box max-w-400">
			<h2 class="text-xl text-center">Agent</h2>
			<table class="table">
				<tbody>
					<tr>
						<td>ID</td>
						<td>{selectedAgent.id}</td>
					</tr>
					<tr>
						<td>Name</td>
						<td>{selectedAgent.name}</td>
					</tr>
					<tr>
						<td>Created At</td>
						<td>{formatTime(selectedAgent.created_at)}</td>
					</tr>
					<tr>
						<td>State</td>
						<td>{selectedAgent.state}</td>
					</tr>
					<tr>
						<td>Pending</td>
						<td>{selectedAgent.pending}</td>
					</tr>
					<tr>
						<td>Memory Limit</td>
						<td>
							{selectedAgent.memory_limit}
							<button
								type="button"
								class="btn"
								onclick={async () => {
									await updateContainer(selectedAgent.id, parseInt(prompt("Enter requested memory size in GiB") || "", 10), 0);
								}}
							>
								<SquarePen />
							</button>
						</td>
					</tr>
					<tr>
						<td>Storage Size</td>
						<td>
							{selectedAgent.storage_size}
							<button
								type="button"
								class="btn"
								onclick={async () => {
									await updateContainer(selectedAgent.id, 0, parseInt(prompt("Enter requested storage size in GiB") || "", 10));
								}}
							>
								<SquarePen />
							</button>
						</td>
					</tr>
				</tbody>
			</table>
			<div class="divider"></div>
			<h3 class="text-lg">Forge Credentials</h3>
			<form
				onsubmit={async (e) => {
					e.preventDefault();
					await updateAgentForge("forge_token", forgeToken);
					forgeToken = "";
				}}
			>
				<fieldset class="fieldset">
					<label for="forge_token">Forge Token</label>
					<input id="forge_token" type="text" placeholder="Forge Token" class="input w-full" bind:value={forgeToken}>
					<div class="flex gap-2">
						<button type="submit" class="btn btn-primary" disabled={!forgeToken}>Save Token</button>
						<button type="button" class="btn" disabled={!selectedAgent.forge_token_configured} onclick={async () => await updateAgentForge("forge_token", "")}>Clear Token</button>
					</div>
				</fieldset>
			</form>
			<form
				onsubmit={async (e) => {
					e.preventDefault();
					await updateAgentForge("forge_webhook_secret", forgeWebhookSecret);
					forgeWebhookSecret = "";
				}}
			>
				<fieldset class="fieldset">
					<label for="forge_webhook_secret">Forge Webhook Secret</label>
					<input id="forge_webhook_secret" type="text" placeholder="Forge Webhook Secret" class="input w-full" bind:value={forgeWebhookSecret}>
					<div class="flex gap-2">
						<button type="submit" class="btn btn-primary" disabled={!forgeWebhookSecret}>Save Webhook Secret</button>
						<button type="button" class="btn" disabled={!selectedAgent.forge_webhook_secret_configured} onclick={async () => await updateAgentForge("forge_webhook_secret", "")}>
							Clear Webhook Secret
						</button>
					</div>
				</fieldset>
			</form>
			<div class="divider"></div>
			<form method="dialog">
				<button
					type="submit"
					class="btn"
					onclick={async () => {
						await setAgentState(selectedAgent.id, "delete");
					}}
				>
					<Trash />
				</button>
			</form>
		</div>
	{/if}
</dialog>
<dialog
	bind:this={creationDialog}
	class="modal"
	onclose={() => {
		newName = "";
	}}
>
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	<div class="modal-box max-w-400">
		<h2 class="text-xl text-center">Create Agent</h2>
		<br>
		<form
			method="dialog"
			onsubmit={async () => {
				await createAgent();
			}}
		>
			<fieldset class="fieldset">
				<div>
					<label for="agent_name">Name</label>
					<input id="agent_name" type="text" placeholder="Name" class="input" required bind:value={newName}>
				</div>
				<button type="submit" class="btn btn-primary">Create</button>
			</fieldset>
		</form>
	</div>
</dialog>
