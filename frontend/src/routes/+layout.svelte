<script lang="ts">
  import "../app.css";
  import { Code, LogOut, MessageSquare, Server, Settings, User } from "@lucide/svelte";
  import { page } from "$app/state";
  import { invalidateAll } from "$app/navigation";
  import { Toaster } from "svelte-sonner";
  import { connectWS, wsState } from "$lib/ws.svelte.js";
  let { children, data } = $props();

  function logout() {
    localStorage.clear();
    location.reload();
  }

  setInterval(async () => {
    await invalidateAll();
  }, 600000);
  setInterval(async () => {
    await connectWS();
  }, 5000);
</script>

<div class="drawer drawer-open h-screen">
  <input id="sidebar" type="checkbox" class="drawer-toggle" />
  <div class="drawer-content p-10 overflow-auto">
    {@render children()}
  </div>
  {#if page.url.pathname !== "/login"}
    <div class="drawer-side">
      <ul class="menu bg-base-200 min-h-full">
        <li>
          <a class="text-2xl" href="/">
            <img src="/logo.png" alt="logo" class="h-15" /> Atlas
          </a>
        </li>
        <li><a href="/devices"><Server />Devices</a></li>
        <li><a href="/commands"><Code />Commands</a></li>
        <li><a href="/chats"><MessageSquare />Chats</a></li>
        {#if localStorage.getItem("admin") === "true"}
          <li><a href="/settings"><Settings />Settings</a></li>
        {/if}
        <li class="mt-auto"><a href="/profile"><User />Profile</a></li>
        <li><button onclick={logout}><LogOut />Logout</button></li>
        <li>
          <div class="text-xs">
            <div class:status-success={data.connected && wsState.connected} class:status-error={!data.connected || !wsState.connected} class="status"></div>
            {data.info.hostname}
            <br />
            Version {data.info.version || "Unknown"}
          </div>
        </li>
      </ul>
    </div>
  {/if}
</div>
<Toaster theme="system" richColors />
