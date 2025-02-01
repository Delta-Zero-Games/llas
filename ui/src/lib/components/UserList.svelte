<!-- ui/src/lib/components/UserList.svelte -->
<script lang="ts">
    import { roomStore } from '../stores/roomStore';
    import { audioStore } from '../stores/audioStore';
    import { Mic, MicOff, Volume2, VolumeX } from 'lucide-svelte';
  
    // Individual volume controls for users
    let userVolumes = new Map<string, number>();
  
    $: currentRoom = $roomStore.currentRoom;
    $: participants = currentRoom?.participants || [];
  
    function handleVolumeChange(userId: string, volume: number) {
      userVolumes.set(userId, volume);
      // Update the audio system with new volume
      audioStore.setUserVolume(userId, volume);
    }
  </script>
  
  <div class="space-y-3">
    {#if !currentRoom}
      <div class="text-gray-500 text-center py-4">
        Join a room to see participants
      </div>
    {:else}
      <div class="text-sm font-medium text-gray-300">
        {currentRoom.name} - {participants.length} participants
      </div>
      
      <div class="space-y-2">
        {#each participants as user}
          <div class="flex items-center justify-between p-3 bg-gray-700 rounded-lg">
            <!-- User Info -->
            <div class="flex items-center gap-2">
              <!-- Status Icon -->
              {#if user.is_muted}
                <MicOff class="w-4 h-4 text-red-500" />
              {:else}
                <Mic class="w-4 h-4 text-green-500" />
              {/if}
              
              <!-- Username -->
              <span class="text-white">
                {user.name}
                {#if user.id === currentRoom.creator_id}
                  <span class="text-xs text-blue-400">(host)</span>
                {/if}
              </span>
            </div>
  
            <!-- Volume Control -->
            <div class="flex items-center gap-3">
              <input
                type="range"
                min="0"
                max="1"
                step="0.1"
                value={userVolumes.get(user.id) ?? 1}
                on:input={(e) => handleVolumeChange(user.id, e.currentTarget.valueAsNumber)}
                class="w-24 accent-blue-500"
              />
              
              {#if user.is_deafened}
                <VolumeX class="w-4 h-4 text-red-500" />
              {:else}
                <Volume2 class="w-4 h-4 text-green-500" />
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>