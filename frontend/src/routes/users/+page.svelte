<script lang="ts">
  import { Check, Plus, Trash, X } from "@lucide/svelte";
  import { authenticatedFetch } from "$lib/auth.ts";
  import { encryptToServer } from "$lib/util.ts";

  let { data } = $props();
  let selectedUser: any = $state();
  let dialog: any = $state();
  let creationDialog: any = $state();
  let newUsername = $state("");
  let newPassword = $state("");
  let newPasswordConfirmation = $state("");
  let newAdmin = $state(false);

  async function createUser() {
    if (newPassword != newPasswordConfirmation) {
      alert("Password fields don't match");
      return;
    }
    const body = JSON.stringify({ username: newUsername, password: newPassword, admin: newAdmin });
    await authenticatedFetch(
      `/api/users`,
      {
        method: "POST",
        body: await encryptToServer(body),
      },
      undefined,
      "application/octet-stream",
    );
    newUsername = "";
    newPassword = "";
    newPasswordConfirmation = "";
    newAdmin = false;
  }
  async function deleteUser(id: string) {
    await authenticatedFetch(`/api/users/${id}`, {
      method: "DELETE",
    });
  }
</script>

<table class="border border-base-content/10 table">
  <thead>
    <tr>
      <th>Admin</th>
      <th>Username</th>
    </tr>
  </thead>
  <tbody>
    {#each data.users as user}
      <tr
        class="cursor-pointer hover:bg-base-200"
        onclick={() => {
          selectedUser = user;
          dialog.showModal();
        }}
      >
        <td>
          {#if user.admin}
            <Check />
          {:else}
            <X />
          {/if}
        </td>
        <td>{user.username}</td>
      </tr>
    {/each}
  </tbody>
</table>
<div class="fab">
  <button
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
    <button>Close</button>
  </form>
  {#if selectedUser}
    <div class="modal-box max-w-200">
      <h2 class="text-xl text-center">User</h2>
      <table class="table">
        <tbody>
          <tr>
            <td>ID</td>
            <td>{selectedUser.id}</td>
          </tr>
          <tr>
            <td>Username</td>
            <td>{selectedUser.username}</td>
          </tr>
          <tr>
            <td>Admin</td>
            <td>{selectedUser.admin}</td>
          </tr>
        </tbody>
      </table>
      <div class="divider"></div>
      <form method="dialog">
        <button
          class="btn"
          onclick={async () => {
            await deleteUser(selectedUser.id);
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
    <button>Close</button>
  </form>
  <div class="modal-box max-w-400">
    <h2 class="text-xl text-center">Create User</h2>
    <br />
    <form
      method="dialog"
      onsubmit={async () => {
        await createUser();
      }}
    >
      <fieldset class="fieldset">
        <input id="username" type="text" placeholder="Username" class="input" required autocomplete="username" bind:value={newUsername} />
        <input id="password" type="password" placeholder="Password" class="input" required autocomplete="new-password" bind:value={newPassword} />
        <input id="passwordConfirmation" type="password" placeholder="Confirm password" class="input" required autocomplete="new-password" bind:value={newPasswordConfirmation} />
        <p>
          Admin:
          <input id="admin" type="checkbox" class="checkbox" bind:checked={newAdmin} />
        </p>
        <button class="btn btn-primary">Create</button>
      </fieldset>
    </form>
  </div>
</dialog>
