<script lang="ts">
  import { authenticatedFetch } from "$lib/auth.ts";
  import { encryptToServer, formatTime } from "$lib/util.ts";
  import { Trash } from "@lucide/svelte";
  import { toast } from "svelte-sonner";

  let { data } = $props();
  let passwordDialog: any = $state();
  let oldPassword = $state("");
  let newPassword = $state("");
  let passwordConfirmation = $state("");

  async function changePassword() {
    if (newPassword != passwordConfirmation) {
      alert("New password fields don't match");
      return;
    }
    const body = JSON.stringify({ old_password: oldPassword, new_password: newPassword });
    const response = await authenticatedFetch(
      `/api/user/update_password`,
      {
        method: "POST",
        body: await encryptToServer(body),
      },
      undefined,
      "application/octet-stream",
    );
    if (response.ok) {
      passwordDialog.close();
      toast.success("Changed password");
      localStorage.removeItem("jwt");
      localStorage.removeItem("privkey");
      location.reload();
    } else {
      toast.error("Request denied");
    }
  }
  async function registerPasskey() {
    const body = JSON.stringify({ password: prompt("Enter your password to register a new passkey:") });
    const encryptedBody = await encryptToServer(body);
    const startRes = await authenticatedFetch(`/api/passkeys/start_registration`, { method: "POST", body: encryptedBody, headers: { "Content-Type": "application/octet-stream" } });
    const startData = await startRes.json();
    const credential = await navigator.credentials.create({ publicKey: PublicKeyCredential.parseCreationOptionsFromJSON(startData["publicKey"]) });
    await authenticatedFetch(`/api/passkeys/finish_registration`, {
      method: "POST",
      body: JSON.stringify({ credential, name: prompt("Enter a name for your passkey:") }),
    });
  }
  async function deletePasskey(id: string) {
    await authenticatedFetch(`/api/passkeys/${id}`, {
      method: "DELETE",
    });
  }
</script>

<button
  class="btn"
  onclick={() => {
    passwordDialog.showModal();
  }}
>
  Change Password
</button>
<dialog bind:this={passwordDialog} class="modal">
  <form class="modal-backdrop" method="dialog">
    <button>Close</button>
  </form>
  <div class="modal-box max-w-200">
    <h1>Change Password</h1>
    <form
      onsubmit={async () => {
        await changePassword();
      }}
    >
      <fieldset class="fieldset">
        <input id="oldPassword" type="password" placeholder="Old password" class="input" required autocomplete="current-password" bind:value={oldPassword} />
        <input id="newPassword" type="password" placeholder="New password" class="input" required autocomplete="new-password" bind:value={newPassword} />
        <input id="passwordConfirmation" type="password" placeholder="Confirm password" class="input" required autocomplete="new-password" bind:value={passwordConfirmation} />
        <button class="btn btn-primary">Update</button>
      </fieldset>
    </form>
  </div>
</dialog>
<br />
<button
  class="btn"
  onclick={async () => {
    await registerPasskey();
  }}
>
  Register Passkey
</button>
<div class="divider"></div>
<table class="border border-base-content/10 table">
  <thead>
    <tr>
      <th>Name</th>
      <th>Created At</th>
      <th>Last Used</th>
      <th>Delete</th>
    </tr>
  </thead>
  <tbody>
    {#each data.passkeys as passkey}
      <tr class="hover:bg-base-200">
        <td>{passkey.name}</td>
        <td>{formatTime(passkey.created_at)}</td>
        {#if passkey.last_used}
          <td>{formatTime(passkey.last_used)}</td>
        {:else}
          <td>Never</td>
        {/if}
        <td>
          <button
            class="btn"
            onclick={async () => {
              await deletePasskey(passkey.id);
            }}
          >
            <Trash />
          </button>
        </td>
      </tr>
    {/each}
  </tbody>
</table>
