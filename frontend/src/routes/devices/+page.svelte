<script lang="ts">
import { Check, ShieldCheck, ShieldQuestionMark, Trash, Unplug } from "@lucide/svelte";
import { toast } from "svelte-sonner";
import { authenticatedFetch } from "#lib/auth.ts";
import { user } from "#lib/state.svelte.ts";
import { formatTime } from "#lib/util.ts";

let { data } = $props();
let selectedDeviceId = $state();
let selectedDevice: any = $derived(data.devices.find((device: any) => device.id === selectedDeviceId));
let dialog: any = $state();

$effect(() => {
	if (selectedDeviceId && !selectedDevice) {
		dialog?.close();
		selectedDeviceId = undefined;
	}
});

async function approveDevice(id: string, expected_pubkey: string) {
	const pubkey = prompt("Enter public key");
	if (pubkey === expected_pubkey) {
		await authenticatedFetch(`/api/devices/${id}/approve`, {
			method: "POST",
		});
	} else {
		toast.error("Invalid Public Key");
	}
}
async function deleteDevice(id: string) {
	await authenticatedFetch(`/api/devices/${id}`, {
		method: "DELETE",
	});
}
</script>

<table class="border border-base-content/10 table">
	<thead>
		<tr>
			<th>State</th>
			<th>Hostname</th>
			<th>Architecture</th>
			<th>OS</th>
			<th>Version</th>
			<th>IP</th>
			<th>First Connection</th>
			<th>Last Connection</th>
		</tr>
	</thead>
	<tbody>
		{#each data.devices as device}
			<tr
				class="cursor-pointer hover:bg-base-200"
				onclick={() => {
					selectedDeviceId = device.id;
					dialog.showModal();
				}}
			>
				<td>
					{#if !device.approved}
						<ShieldQuestionMark />
					{:else if Date.now() / 1000 - device.last_connection > 120}
						<Unplug />
					{:else}
						<Check />
					{/if}
				</td>
				<td>{device.hostname}</td>
				<td>{device.arch}</td>
				<td>{device.os}</td>
				<td>{device.version}</td>
				<td>{device.ip}</td>
				<td>{formatTime(device.first_connection)}</td>
				<td>{formatTime(device.last_connection)}</td>
			</tr>
		{/each}
	</tbody>
</table>
<dialog bind:this={dialog} class="modal">
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	{#if selectedDevice}
		<div class="modal-box max-w-200">
			<h2 class="text-xl text-center">Device</h2>
			<table class="table">
				<tbody>
					<tr>
						<td>ID</td>
						<td>{selectedDevice.id}</td>
					</tr>
					<tr>
						<td>Hostname</td>
						<td>{selectedDevice.hostname}</td>
					</tr>
					<tr>
						<td>Architecture</td>
						<td>{selectedDevice.arch}</td>
					</tr>
					<tr>
						<td>OS</td>
						<td>{selectedDevice.os}</td>
					</tr>
					<tr>
						<td>Version</td>
						<td>{selectedDevice.version}</td>
					</tr>
					<tr>
						<td>Approved</td>
						<td>{selectedDevice.approved}</td>
					</tr>
					<tr>
						<td>First Connection</td>
						<td>{formatTime(selectedDevice.first_connection)}</td>
					</tr>
					<tr>
						<td>Last Connection</td>
						<td>{formatTime(selectedDevice.last_connection)}</td>
					</tr>
					<tr>
						<td>IP</td>
						<td>{selectedDevice.ip}</td>
					</tr>
					<tr>
						<td>Public Key</td>
						<td>{selectedDevice.pubkey}</td>
					</tr>
				</tbody>
			</table>
			<div class="divider"></div>
			{#if user.admin}
				<form method="dialog">
					<button
						type="submit"
						class="btn"
						class:btn-disabled={selectedDevice.approved}
						onclick={async () => {
							await approveDevice(selectedDevice.id, selectedDevice.pubkey);
						}}
					>
						<ShieldCheck />
					</button>
					<button
						type="submit"
						class="btn"
						onclick={async () => {
							await deleteDevice(selectedDevice.id);
						}}
					>
						<Trash />
					</button>
				</form>
			{/if}
		</div>
	{/if}
</dialog>
