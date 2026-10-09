<script lang="ts">
import { CircleQuestionMark, ExternalLink, Loader, Pause, Play, Plus, Power, SquarePen, Trash } from "@lucide/svelte";
import { authenticatedFetch } from "#lib/auth.ts";
import { updateContainer } from "#lib/container.ts";
import { formatTime } from "#lib/util.ts";

let { data } = $props();
let selectedAgent: any = $state();
let dialog: any = $state();
let creationDialog: any = $state();
let newName = $state("");

async function createAgent() {
	await authenticatedFetch(`/api/agents`, {
		method: "POST",
		body: JSON.stringify({ name: newName }),
	});
	newName = "";
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
					selectedAgent = agent;
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
<dialog bind:this={dialog} class="modal">
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
							{selectedAgent.storage_size}
							<button
								type="button"
								class="btn"
								onclick={async () => {
									await updateContainer(selectedAgent.id, 0, parseInt(prompt("Enter requested storage size in GiB") || "", 10));
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
						await setAgentState(selectedAgent.id, "delete");
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
		<h2 class="text-xl text-center">Create Agent</h2>
		<br>
		<form
			method="dialog"
			onsubmit={async () => {
				await createAgent();
			}}
		>
			<fieldset class="fieldset">
				<input id="agent_name" type="text" placeholder="Name" class="input" required bind:value={newName}>
				<button type="submit" class="btn btn-primary">Create</button>
			</fieldset>
		</form>
	</div>
</dialog>
