<!-- ui/src/App.svelte -->
<script lang="ts">
  import RoomList from './lib/components/RoomList.svelte';
  import UserList from './lib/components/UserList.svelte';
  import AudioDeviceManager from './lib/components/AudioDeviceManager.svelte';
  import UserSetup from './lib/components/UserSetup.svelte';
  import { audioStore } from './lib/stores/audioStore';
  import { roomStore } from './lib/stores/roomStore';
  import { userStore } from './lib/stores/userStore';
</script>

<main class="h-screen flex flex-col bg-zinc-900">
  <!-- Show user setup if no user is set -->
  {#if !$userStore.currentUser}
    <UserSetup />
  {/if}

  <!-- Header -->
  <header class="bg-zinc-800 shadow-sm">
    <div class="container mx-auto px-4 py-3 flex items-center justify-between">
      <div class="flex items-center gap-3">
        <img src="/logo.svg" class="w-10 h-10" alt="LLAS logo" />
        <h1 class="text-xl font-semibold text-white">LLAS</h1>
      </div>
      
      <!-- User and Connection Status -->
      <div class="flex items-center gap-4">
        {#if $userStore.currentUser}
          <div class="flex items-center gap-2">
            <span class="text-sm text-zinc-300">
              {$userStore.currentUser.name}
            </span>
            <button 
              class="text-xs text-zinc-400 hover:text-white"
              on:click={() => userStore.updateName(prompt('Enter new name:') || $userStore.currentUser?.name || '')}
            >
              Edit
            </button>
          </div>
        {/if}

        <div class="flex items-center gap-2">
          <span class={`w-2 h-2 rounded-full ${$audioStore.isConnected ? 'bg-[#3cf281]' : 'bg-red-500'}`}></span>
          <span class="text-sm text-zinc-300">
            {$audioStore.isConnected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
      </div>
    </div>
  </header>

  <!-- Main Content -->
  <div class="flex-1 container mx-auto px-4 py-6 flex gap-4">
    <!-- Left Sidebar - Room List -->
    <div class="w-64 flex-shrink-0">
      <RoomList />
    </div>

    <!-- Main Content Area -->
    <div class="flex-1 flex flex-col gap-4">
      <UserList />
    </div>

    <!-- Right Sidebar - Audio Controls -->
    <div class="w-80 flex-shrink-0">
      <AudioDeviceManager />
    </div>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen-Sans, Ubuntu, Cantarell, 'Helvetica Neue', sans-serif;
  }
</style>