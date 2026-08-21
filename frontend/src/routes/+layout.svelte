<script lang="ts">
import "../app.css";
import { Bot, Code, LogOut, MessageSquare, Package, Server, Settings, User, Users } from "@lucide/svelte";
import { page } from "$app/state";
import { invalidateAll } from "$app/navigation";
import { Toaster } from "svelte-sonner";
import { connectWS, mounted, user, wsState } from "$lib/state.svelte.ts";
import { getDatabase } from "$lib/util.ts";
import { onMount } from "svelte";
import { App } from "@capacitor/app";
let { children, data } = $props();

async function logout() {
  localStorage.clear();
  const db = getDatabase();
  await db.delete();
  location.reload();
}

setInterval(async () => {
  await invalidateAll();
}, 600000);
setInterval(async () => {
  await connectWS();
}, 5000);

onMount(() => {
  App.addListener("backButton", ({ canGoBack }) => {
    if (canGoBack) {
      history.back();
    } else {
      App.exitApp();
    }
  });
  mounted.mounted = true;
});
</script>

<div class="drawer drawer-open h-screen">
  <input id="sidebar" type="checkbox" class="drawer-toggle" />
  <div class="drawer-content p-10 overflow-auto">
    {#if data.client_error}
      <p>Incorrect client for this server, reload the page or reinstall the client.</p>
    {:else if page.url.pathname === "/login" || localStorage.getItem("jwt")}
      {@render children()}
    {/if}
  </div>
  {#if page.url.pathname !== "/login" && !page.url.pathname.startsWith("/embed/") && !data.client_error}
    <div class="drawer-side">
      <ul class="menu bg-base-200 min-h-full">
        <li>
          <a class="text-2xl" href="/">
            <img src="/logo.png" alt="logo" class="h-12" /><span class="hidden lg:block"> Atlas</span>
          </a>
        </li>
        <li><a href="/devices"><Server /><span class="hidden lg:block">Devices</span></a></li>
        <li><a href="/commands"><Code /><span class="hidden lg:block">Commands</span></a></li>
        <li><a href="/chats"><MessageSquare /><span class="hidden lg:block">Chats</span></a></li>
        <li><a href="/development_environments"><Package /><span class="hidden lg:block">DevEnvs</span></a></li>
        <li><a href="/agents"><Bot /><span class="hidden lg:block">Agents</span></a></li>
        <li><a href="/settings"><Settings /><span class="hidden lg:block">Settings</span></a></li>
        {#if user.admin}
          <li><a href="/users"><Users /><span class="hidden lg:block">Users</span></a></li>
        {/if}
        <li class="mt-auto"><a href="/profile"><User /><span class="hidden lg:block">{user.username}</span></a></li>
        <li>
          <button
            onclick={async () => {
              await logout();
            }}><LogOut /><span class="hidden lg:block">Logout</span></button
          >
        </li>
        <li>
          <div class="text-xs" class:bg-red-500={data.info.environment !== "production"}>
            <div class:status-success={data.connected && wsState.connected} class:status-error={!data.connected || !wsState.connected} class="status"></div>
            <span class="hidden lg:block">
              {data.info.hostname}
              <br />
              {data.info.version || "Unknown version"}
            </span>
          </div>
        </li>
      </ul>
    </div>
  {/if}
</div>
<Toaster theme="system" richColors />
