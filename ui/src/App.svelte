<!-- ui/src/App.svelte -->
<script lang="ts">
  import RoomList from './lib/components/RoomList.svelte';
  import UserList from './lib/components/UserList.svelte';
  import AudioDeviceManager from './lib/components/AudioDeviceManager.svelte';
  import { audioStore } from './lib/stores/audioStore';
  import { roomStore } from './lib/stores/roomStore';
</script>

<main class="h-screen flex flex-col bg-gray-900">
  <!-- Header -->
  <header class="bg-gray-800 shadow-sm">
    <div class="container mx-auto px-4 py-3 flex items-center justify-between">
      <div class="flex items-center gap-3">
        <img src="/logo.svg" class="w-10 h-10" alt="LLAS logo" />
        <h1 class="text-xl font-semibold text-white">LLAS</h1>
      </div>
      
      <!-- Connection Status -->
      <div class="flex items-center gap-2">
        <span class={`w-2 h-2 rounded-full ${$audioStore.isConnected ? 'bg-green-500' : 'bg-red-500'}`}></span>
        <span class="text-sm text-gray-300">
          {$audioStore.isConnected ? 'Connected' : 'Disconnected'}
        </span>
      </div>
    </div>
  </header>

  <!-- Main Content -->
  <div class="flex-1 container mx-auto px-4 py-6 flex gap-4">
    <!-- Left Sidebar - Room List -->
    <aside class="w-64 bg-gray-800 rounded-lg shadow-sm p-4">
      <RoomList />
    </aside>

    <!-- Main Area - Current Room & Participants -->
    <div class="flex-1 bg-gray-800 rounded-lg shadow-sm p-4">
      <UserList />
    </div>

    <!-- Right Sidebar - Audio Controls -->
    <aside class="w-64 bg-gray-800 rounded-lg shadow-sm p-4">
      <AudioDeviceManager />
    </aside>
  </div>
</main>