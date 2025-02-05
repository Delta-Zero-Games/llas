<!-- ui/src/lib/components/RoomList.svelte -->
<script lang="ts">
  import { roomStore, type Room } from '../stores/roomStore';
  import { networkStore } from '../stores/networkStore';
  import { userStore } from '../stores/userStore';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';  // Added this import

  let newRoomName = '';
  let isCreatingRoom = false;
  let isLeavingRoom = false;

  // Refresh room list on mount and periodically
  async function cleanupStaleRooms() {
      try {
          await invoke('cleanup_rooms');
          await roomStore.refreshRooms();
      } catch (err) {
          console.error('Failed to cleanup rooms:', err);
      }
  }

  onMount(() => {
      // Run cleanup immediately
      cleanupStaleRooms();
      roomStore.refreshRooms();
      
      // Set up periodic refresh
      const interval = setInterval(() => {
          roomStore.refreshRooms();
      }, 5000);

      // Return the cleanup function
      return () => clearInterval(interval);
  });

  async function handleCreateRoom() {
      if (!newRoomName.trim() || !$userStore.currentUser) return;
      
      isCreatingRoom = true;
      try {
          // Create room and automatically join it
          const room = await roomStore.createRoom(newRoomName, $userStore.currentUser.id);
          console.log('Room created:', room);

          // Start streaming for the new room
          await networkStore.startStreaming(room.id);
          console.log('Streaming started for new room');

          // Clear input after success
          newRoomName = '';
      } catch (err) {
          console.error('Failed to create room:', err);
          const errorMessage = err instanceof Error ? err.message : 'Failed to create room';
          alert(errorMessage);
      } finally {
          isCreatingRoom = false;
      }
  }

  async function handleJoinRoom(roomId: string) {
      if (!$userStore.currentUser) return;
      
      try {
          console.log('Joining room:', roomId);
          await roomStore.joinRoom(roomId, $userStore.currentUser.id);
          console.log('Successfully joined room');

          console.log('Starting streaming for joined room');
          await networkStore.startStreaming(roomId);
          console.log('Streaming started');
      } catch (err) {
          console.error('Failed to join room:', err);
          alert(err instanceof Error ? err.message : 'Failed to join room');
      }
  }

  async function handleLeaveRoom(roomId: string) {
      if (!$userStore.currentUser) return;
      
      isLeavingRoom = true;
      try {
          console.log('Stopping streaming before leaving room');
          await networkStore.stopStreaming();

          console.log('Leaving room:', roomId);
          await roomStore.leaveRoom(roomId, $userStore.currentUser.id);
          console.log('Successfully left room');
      } catch (err) {
          console.error('Failed to leave room:', err);
          alert(err instanceof Error ? err.message : 'Failed to leave room');
      } finally {
          isLeavingRoom = false;
      }
  }

  // Filter rooms - only show rooms with participants
  $: activeRooms = $roomStore.rooms.filter(room => room.participants.length > 0);
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
                                  disabled={$roomStore.isLoading}
                                  class="py-1 px-3 bg-[#3cf281] hover:bg-[#34d973] disabled:opacity-50 disabled:cursor-not-allowed text-zinc-900 text-sm font-medium rounded-md transition-colors"
                              >
                                  {$roomStore.isLoading ? 'Joining...' : 'Join'}
                              </button>
                          {/if}
                      </div>

                      <!-- Participant List -->
                      <div class="space-y-1">
                          {#each room.participants as participant}
                              <div class="flex items-center gap-2 text-sm">
                                  <span class="text-[#3cf281]">{participant.name}</span>
                              </div>
                          {/each}
                      </div>
                  </div>
              {/each}
          </div>
      {/if}

      {#if $roomStore.error}
          <div class="mt-4 p-3 bg-red-500/10 border border-red-500 rounded-lg">
              <p class="text-red-500 text-sm">{$roomStore.error}</p>
          </div>
      {/if}
  </div>
</div>