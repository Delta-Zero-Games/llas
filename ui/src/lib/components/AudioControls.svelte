<!-- ui/src/lib/components/AudioControls.svelte -->
<script lang="ts">
    let isMuted = false;
    let isDeafened = false;
    let volume = 100;
</script>

<div class="space-y-4 text-white">
    <div class="flex flex-col gap-2">
        <label for="volume-control" class="text-sm text-gray-300">Volume</label>
        <input 
        id="volume-control"
        type="range" 
        bind:value={volume} 
        min="0" 
        max="100" 
        class="w-full accent-blue-600"
        />
    </div>

    <div class="flex gap-2">
            <button 
            class={`flex-1 py-2 px-4 rounded-lg transition-colors ${isMuted ? 'bg-red-500 text-white' : 'bg-gray-700'}`}
            on:click={() => isMuted = !isMuted}
            >
            Mute
        </button>
        
        <button 
            class={`flex-1 py-2 px-4 rounded-lg transition-colors ${isDeafened ? 'bg-red-500 text-white' : 'bg-gray-700'}`}
            on:click={() => isDeafened = !isDeafened}
            >
            Deafen
        </button>
    </div>
</div>