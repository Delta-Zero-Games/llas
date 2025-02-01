<!-- ui/src/lib/components/RoomList.svelte -->
<script lang="ts">
    import { roomStore, type Room } from '../stores/roomStore';
    import { audioStore } from '../stores/audioStore';
    import { onMount } from 'svelte';
  
    let newRoomName = '';
    let isCreatingRoom = false;
    let userId = crypto.randomUUID(); // Temporary user ID for testing
  
    // Refresh room list on mount
    onMount(() => {
      roomStore.refreshRooms();
      // Set up periodic refresh
      const interval = setInterval(() => {
        roomStore.refreshRooms();
      }, 5000);
  
      return () => clearInterval(interval);
    });
  
    async function handleCreateRoom() {
      if (!newRoomName.trim()) return;
      
      isCreatingRoom = true;
      try {
        await roomStore.createRoom(newRoomName, userId);
        newRoomName = ''; // Clear input after success
      } finally {
        isCreatingRoom = false;
      }
    }
  
    async function handleJoinRoom(roomId: string) {
      await roomStore.joinRoom(roomId, userId);
      // Start audio when joining a room
      await audioStore.startAudio('your-turn-server:port');
    }
  </script>
  
  <div class="space-y-4">
    <!-- Create Room Form -->
    <div class="space-y-2">
      <input
        type="text"
        bind:value={newRoomName}
        placeholder="Room name"
        class="w-full px-3 py-2 bg-gray-700 rounded-lg focus:ring-2 focus:ring-blue-500 outline-none"
      />
      <button
        on:click={handleCreateRoom}
        disabled={isCreatingRoom || !newRoomName.trim()}
        class="w-full py-2 px-4 bg-blue-600 text-white rounded-lg hover:bg-blue-700 
               transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {isCreatingRoom ? 'Creating...' : 'Create Room'}
      </button>
    </div>
  
    <!-- Room List -->
    <div class="space-y-2">
      <h3 class="text-sm font-medium text-gray-300">Available Rooms</h3>
      
      {#if $roomStore.rooms.length === 0}
        <div class="text-sm text-gray-500 text-center py-4">
          No rooms available
        </div>
      {:else}
        {#each $roomStore.rooms as room}
          <div class="p-3 bg-gray-700 rounded-lg space-y-2">
            <div class="flex items-center justify-between">
              <span class="text-white font-medium">{room.name}</span>
              <span class="text-sm text-gray-400">
                {room.participants.length} users
              </span>
            </div>
            
            {#if $roomStore.currentRoom?.id === room.id}
              <button
                on:click={() => roomStore.leaveRoom(room.id, userId)}
                class="w-full py-1.5 px-3 bg-red-500 text-white rounded
                       hover:bg-red-600 transition-colors text-sm"
              >
                Leave Room
              </button>
            {:else}
              <button
                on:click={() => handleJoinRoom(room.id)}
                class="w-full py-1.5 px-3 bg-blue-500 text-white rounded
                       hover:bg-blue-600 transition-colors text-sm"
              >
                Join Room
              </button>
            {/if}
          </div>
        {/each}
      {/if}
      
      <!-- Error Display -->
      {#if $roomStore.error}
        <div class="p-3 bg-red-500/20 border border-red-500 rounded-lg text-red-500 text-sm">
          {$roomStore.error}
        </div>
      {/if}
    </div>
  </div>