<!-- ui/src/lib/components/RoomList.svelte -->
<script lang="ts">
    import { roomStore, type Room } from '../stores/roomStore';
    import { networkStore } from '../stores/networkStore';
    import { userStore } from '../stores/userStore';
    import { onMount } from 'svelte';
  
    let newRoomName = '';
    let isCreatingRoom = false;
    let isLeavingRoom = false;
  
    // Refresh room list on mount and periodically
    onMount(() => {
      roomStore.refreshRooms();
      // Set up periodic refresh
      const interval = setInterval(() => {
        roomStore.refreshRooms();
      }, 5000);
  
      return () => clearInterval(interval);
    });
  
    async function handleCreateRoom() {
      if (!newRoomName.trim() || !$userStore.currentUser) return;
      
      isCreatingRoom = true;
      try {
        const room = await roomStore.createRoom(newRoomName, $userStore.currentUser.id);
        newRoomName = ''; // Clear input after success
        await networkStore.startStreaming(room.id);
      } catch (err) {
        console.error('Failed to create room:', err);
        // Show error to user (you might want to add a toast or error message UI)
        alert(err instanceof Error ? err.message : 'Failed to create room');
      } finally {
        isCreatingRoom = false;
      }
    }
  
    async function handleJoinRoom(roomId: string) {
      if (!$userStore.currentUser) return;
      
      try {
        await roomStore.joinRoom(roomId, $userStore.currentUser.id);
        await networkStore.startStreaming(roomId);
      } catch (err) {
        console.error('Failed to join room:', err);
      }
    }

    async function handleLeaveRoom(roomId: string) {
      if (!$userStore.currentUser) return;
      
      isLeavingRoom = true;
      try {
        await networkStore.stopStreaming();
        await roomStore.leaveRoom(roomId, $userStore.currentUser.id);
      } catch (err) {
        console.error('Failed to leave room:', err);
      } finally {
        isLeavingRoom = false;
      }
    }

    // Filter out rooms with no participants if they're not the current room
    $: activeRooms = $roomStore.rooms.filter(room => 
      room.id === $roomStore.currentRoom?.id || room.participants.length > 0
    );
</script>
  
<div class="space-y-4">
    <!-- Create Room Form -->
    <div class="space-y-2">
      <input
        type="text"
        bind:value={newRoomName}
        placeholder="Room name"
        class="w-full px-3 py-2 bg-zinc-700 rounded-lg focus:ring-2 focus:ring-[#3cf281] outline-none text-white placeholder-zinc-400"
      />
      <button
        on:click={handleCreateRoom}
        disabled={isCreatingRoom || !newRoomName.trim() || !$userStore.currentUser}
        class="w-full py-2 px-4 bg-[#3cf281] hover:bg-[#34d973] disabled:opacity-50 disabled:cursor-not-allowed text-zinc-900 font-medium rounded-lg transition-colors"
      >
        {isCreatingRoom ? 'Creating...' : 'Create Room'}
      </button>
    </div>

    <!-- Room List -->
    <div class="space-y-2">
      <h2 class="text-lg font-medium text-white">Available Rooms</h2>
      {#if activeRooms.length === 0}
        <p class="text-zinc-400 text-center py-4">No active rooms</p>
      {:else}
        <div class="space-y-2">
          {#each activeRooms as room}
            <div class="p-3 bg-zinc-800 rounded-lg space-y-2">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-white font-medium">{room.name}</h3>
                  <p class="text-sm text-zinc-400">
                    {room.participants.length} {room.participants.length === 1 ? 'participant' : 'participants'}
                  </p>
                </div>
                {#if $roomStore.currentRoom?.id === room.id}
                  <button
                    on:click={() => handleLeaveRoom(room.id)}
                    disabled={isLeavingRoom}
                    class="py-1 px-3 bg-red-500 hover:bg-red-600 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-medium rounded-md transition-colors"
                  >
                    {isLeavingRoom ? 'Leaving...' : 'Leave'}
                  </button>
                {:else}
                  <button
                    on:click={() => handleJoinRoom(room.id)}
                    class="py-1 px-3 bg-[#3cf281] hover:bg-[#34d973] disabled:opacity-50 disabled:cursor-not-allowed text-zinc-900 text-sm font-medium rounded-md transition-colors"
                  >
                    Join
                  </button>
                {/if}
              </div>

              <!-- Participant List -->
              <div class="space-y-1">
                {#each room.participants as participant}
                  <div class="flex items-center gap-2 text-sm">
                    <span class="text-[#3cf281]">{participant.name}</span>
                    {#if participant.id === room.creator_id}
                      <span class="text-xs text-zinc-400">(Host)</span>
                    {/if}
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Error Display -->
      {#if $roomStore.error}
        <div class="mt-4 p-3 bg-red-500/10 border border-red-500 rounded-lg">
          <p class="text-red-500 text-sm">{$roomStore.error}</p>
        </div>
      {/if}
    </div>
</div>