<script lang="ts">
  import { authenticatedFetch } from "$lib/auth";
  import { formatTime } from "$lib/util.ts";
  import { CircleQuestionMark, Loader, Pause, Play, Plus, Trash } from "@lucide/svelte";

  let { data } = $props();
  let selectedEnvironment: any = $state();
  let dialog: any = $state();
  let creationDialog: any = $state();
  let newRepo = $state("");
  let newBranch = $state("");

  async function createEnvironment() {
    await authenticatedFetch(`/api/development_environments`, {
      method: "POST",
      body: JSON.stringify({ repo: newRepo, branch: newBranch }),
    });
    newRepo = "";
    newBranch = "";
  }
  async function deleteEnvironment(id: string) {
    await authenticatedFetch(`/api/development_environments/${id}`, {
      method: "DELETE",
    });
  }
</script>

<table class="border border-base-content/10 table">
  <thead>
    <tr>
      <th>State</th>
      <th>Repo</th>
      <th>Branch</th>
      <th>Created At</th>
    </tr>
  </thead>
  <tbody>
    {#each data.development_environments as environment}
      <tr
        class="cursor-pointer hover:bg-base-200"
        onclick={() => {
          selectedEnvironment = environment;
          dialog.showModal();
        }}
      >
        <td>
          {#if environment.pending}
            <Loader />
          {:else if environment.state === "Running"}
            <Play />
          {:else if environment.state === "Stopped"}
            <Pause />
          {:else if environment.state === "Non-Existent"}
            <Trash />
          {:else}
            <CircleQuestionMark />
          {/if}
        </td>
        <td>{environment.repo}</td>
        <td>{environment.branch}</td>
        <td>{formatTime(environment.created_at)}</td>
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
        </tbody>
      </table>
      <div class="divider"></div>
      <form method="dialog">
        <button
          class="btn"
          onclick={async () => {
            await deleteEnvironment(selectedEnvironment.id);
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
    <h2 class="text-xl text-center">Create Environment</h2>
    <br />
    <form
      method="dialog"
      onsubmit={async () => {
        await createEnvironment();
      }}
    >
      <fieldset class="fieldset">
        <input id="repo" type="text" placeholder="Repository" class="input" required bind:value={newRepo} />
        <input id="branch" type="text" placeholder="Branch" class="input" bind:value={newBranch} />
        <button class="btn btn-primary">Create</button>
      </fieldset>
    </form>
  </div>
</dialog>
