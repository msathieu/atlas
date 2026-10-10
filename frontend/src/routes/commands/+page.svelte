<script lang="ts">
import { Check, Loader, Plus, Trash, X } from "@lucide/svelte";
import DOMPurify from "dompurify";
import hljs from "highlight.js";
import { authenticatedFetch } from "#lib/auth.js";
import { user } from "#lib/state.svelte.ts";
import { formatTime, getHostnameForDevice } from "#lib/util.ts";
import "highlight.js/styles/github-dark.css";

let { data } = $props();
let selectedCommandId = $state();
let selectedCommand: any = $derived(data.commands.find((command: any) => command.id === selectedCommandId));
let dialog: any = $state();
let creationDialog: any = $state();
let newDevice = $state("");
let newCode = $state("");

$effect(() => {
	if (selectedCommandId && !selectedCommand) {
		dialog?.close();
		selectedCommandId = undefined;
	}
});

async function createCommand() {
	await authenticatedFetch(`/api/commands`, {
		method: "POST",
		body: JSON.stringify({ device: newDevice, code: newCode }),
	});
	newDevice = "";
	newCode = "";
}
async function deleteCommand(id: string) {
	await authenticatedFetch(`/api/commands/${id}`, {
		method: "DELETE",
	});
}
function matchHighlightLanguage(language: string): string {
	switch (language) {
		case "Lua":
			return "lua";
		case "Python":
			return "python";
	}
	throw new Error(`Unsupported language`);
}
</script>

<table class="border border-base-content/10 table">
	<thead>
		<tr>
			<th>State</th>
			<th>Origin</th>
			<th>Device</th>
			<th>Request Time</th>
			<th>Status Code</th>
		</tr>
	</thead>
	<tbody>
		{#each data.commands as command}
			<tr
				class="cursor-pointer hover:bg-base-200"
				onclick={() => {
					selectedCommandId = command.id;
					dialog.showModal();
				}}
			>
				<td>
					{#if !command.completed}
						<Loader />
					{:else if command.status_code === 0}
						<Check />
					{:else}
						<X />
					{/if}
				</td>
				<td>{command.origin}</td>
				<td>{getHostnameForDevice(data.devices, command.device)}</td>
				<td>{formatTime(command.requested_timestamp)}</td>
				<td>{command.status_code}</td>
			</tr>
		{/each}
	</tbody>
</table>
{#if user.admin}
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
{/if}
<dialog bind:this={dialog} class="modal">
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	{#if selectedCommand}
		<div class="modal-box max-w-400">
			<h2 class="text-xl text-center">Command</h2>
			<table class="table">
				<tbody>
					<tr>
						<td>ID</td>
						<td>{selectedCommand.id}</td>
					</tr>
					<tr>
						<td>Device</td>
						<td>{selectedCommand.device}</td>
					</tr>
					<tr>
						<td>Completed</td>
						<td>{selectedCommand.completed}</td>
					</tr>
					<tr>
						<td>Request Time</td>
						<td>{formatTime(selectedCommand.requested_timestamp)}</td>
					</tr>
					<tr>
						<td>Language</td>
						<td>{selectedCommand.language}</td>
					</tr>
					<tr>
						<td>Code</td>
						<td>
							<code class="whitespace-pre"
								>{@html DOMPurify.sanitize(
									hljs.highlight(selectedCommand.code, {
										language: matchHighlightLanguage(selectedCommand.language),
									}).value,
								)}</code
							>
						</td>
					</tr>
					{#if selectedCommand.completed}
						<tr>
							<td>Status Code</td>
							<td>{selectedCommand.status_code}</td>
						</tr>
						<tr>
							<td>Completion Time</td>
							<td>{formatTime(selectedCommand.completed_timestamp)}</td>
						</tr>
						<tr>
							<td>Stdout</td>
							<td>
								<code class="whitespace-pre">{selectedCommand.stdout}</code>
							</td>
						</tr>
						<tr>
							<td>Stderr</td>
							<td>
								<code class="whitespace-pre">{selectedCommand.stderr}</code>
							</td>
						</tr>
					{/if}
				</tbody>
			</table>
			<div class="divider"></div>
			<form method="dialog">
				<button
					type="submit"
					class="btn"
					onclick={async () => {
						await deleteCommand(selectedCommand.id);
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
		<h2 class="text-xl text-center">Create Command</h2>
		<br>
		<form
			method="dialog"
			onsubmit={async () => {
				await createCommand();
			}}
		>
			<fieldset class="fieldset">
				<select id="device" class="select w-full" required bind:value={newDevice}>
					<option disabled selected value="">Choose a Device</option>
					{#each data.devices as device}
						{#if device.approved}
							<option value={device.id}>{device.id} - {device.hostname}</option>
						{/if}
					{/each}
				</select>
				<textarea id="code" placeholder="Lua Code" class="textarea w-full" required bind:value={newCode}></textarea>
				<button type="submit" class="btn btn-primary">Create</button>
			</fieldset>
		</form>
	</div>
</dialog>
