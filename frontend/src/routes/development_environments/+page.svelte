<script lang="ts">
import { authenticatedFetch } from "$lib/auth";
import { formatTime } from "$lib/util.ts";
import { CircleQuestionMark, ExternalLink, Loader, Pause, Play, Plus, Power, Trash } from "@lucide/svelte";

let { data } = $props();
let selectedEnvironment: any = $state();
let dialog: any = $state();
let creationDialog: any = $state();
let newName = $state("");
let newRepo = $state("");
let newBranch = $state("");

async function createEnvironment() {
  await authenticatedFetch(`/api/development_environments`, {
    method: "POST",
    body: JSON.stringify({ name: newName, repo: newRepo, branch: newBranch }),
  });
  newName = "";
  newRepo = "";
  newBranch = "";
}
async function deleteEnvironment(id: string) {
  await authenticatedFetch(`/api/development_environments/${id}`, {
    method: "DELETE",
  });
}
async function startEnvironment(id: string) {
  await authenticatedFetch(`/api/development_environments/${id}/start`, {
    method: "POST",
  });
}
async function stopEnvironment(id: string) {
  await authenticatedFetch(`/api/development_environments/${id}/stop`, {
    method: "POST",
  });
}
async function openDevelopmentEnvironment(id: string) {
  const response = await authenticatedFetch(`/api/development_environments/${id}/open`, {
    method: "POST",
  });
  const data = await response.json();
  const url = new URL(data.url);
  if (url.protocol !== "https:") {
    throw new Error("Invalid protocol");
  }
  window.open(data.url, "_blank", "noopener");
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
          selectedEnvironment = environment;
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
          {#if !environment.pending && environment.state === "Running"}
            <button
              class="btn"
              onclick={async (e) => {
                e.stopPropagation();
                await openDevelopmentEnvironment(environment.id);
              }}
            >
              <ExternalLink />
            </button>
            <button
              class="btn"
              onclick={async (e) => {
                e.stopPropagation();
                await stopEnvironment(environment.id);
              }}
            >
              <Power />
            </button>
          {/if}
          {#if !environment.pending && environment.state === "Stopped"}
            <button
              class="btn"
              onclick={async (e) => {
                e.stopPropagation();
                await startEnvironment(environment.id);
              }}
            >
              <Play />
            </button>
          {/if}
        </td>
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
        <input id="name" type="text" placeholder="Name" class="input" required bind:value={newName} />
        <input id="repo" type="text" placeholder="Repository" class="input" bind:value={newRepo} />
        <input id="branch" type="text" placeholder="Branch" class="input" bind:value={newBranch} />
        <button class="btn btn-primary">Create</button>
      </fieldset>
    </form>
  </div>
</dialog>
