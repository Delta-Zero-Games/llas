<!-- ui/src/lib/components/AudioDeviceManager.svelte -->
<!-- ui/src/lib/components/AudioDeviceManager.svelte -->
<script lang="ts">
    import { onMount } from 'svelte';
    import { audioStore } from '../stores/audioStore';
    import AudioMeter from './AudioMeter.svelte';
  
    let stream: MediaStream | null = null;
    let error: string | null = null;
  
    async function handleInputChange(deviceId: string) {
      try {
        if (stream) {
          stream.getTracks().forEach(track => track.stop());
        }
        
        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            deviceId: { exact: deviceId },
            echoCancellation: true,
            noiseSuppression: true,
            autoGainControl: true
          }
        });
  
        // Start the audio system with the selected device
        await audioStore.startAudio("your-turn-server:port");
        
      } catch (err) {
        error = err instanceof Error ? err.message : 'Failed to switch audio input device';
      }
    }
  
    onMount(async () => {
      try {
        await navigator.mediaDevices.getUserMedia({ audio: true });
        const devices = await navigator.mediaDevices.enumerateDevices();
        
        // Set up device change listener
        navigator.mediaDevices.addEventListener('devicechange', () => {
          // Handle device changes
        });
        
      } catch (err) {
        error = 'Failed to access audio devices. Please check permissions.';
      }
    });
  </script>

<div class="space-y-4 p-4 bg-gray-800 rounded-lg">
    {#if error}
        <div class="p-2 bg-red-500 text-white rounded">
            {error}
        </div>
    {/if}

    <div class="space-y-2">
        <label for="input-device" class="text-sm text-gray-300">Input Device</label>
        <select
            id="input-device"
            class="w-full p-2 bg-gray-700 rounded"
            bind:value={selectedInputId}
            on:change={handleInputChange}
        >
            {#each inputDevices as device}
                <option value={device.id}>{device.name}</option>
            {/each}
        </select>
    </div>

    <div class="space-y-2">
        <label for="output-device" class="text-sm text-gray-300">Output Device</label>
        <select
            id="output-device"
            class="w-full p-2 bg-gray-700 rounded"
            bind:value={selectedOutputId}
            on:change={() => {
                const selectedOutput = outputDevices.find(d => d.id === selectedOutputId);
                audioStore.setDevices(null, selectedOutput || null);
            }}
        >
            {#each outputDevices as device}
                <option value={device.id}>{device.name}</option>
            {/each}
        </select>
    </div>

    <!-- Add audio meter -->
    <div class="space-y-2">
        <label class="text-sm text-gray-300">Input Level</label>
        <AudioMeter {stream} />
    </div>

    <!-- Add volume controls -->
    <div class="space-y-2">
        <label class="text-sm text-gray-300">Input Volume</label>
        <input 
            type="range" 
            min="0" 
            max="1" 
            step="0.1"
            class="w-full accent-blue-600"
            bind:value={$audioStore.inputVolume}
            on:input={(e) => audioStore.setVolume('input', e.currentTarget.valueAsNumber)}
        />
    </div>

    <div class="flex gap-2">
        <button 
            class={`flex-1 py-2 px-4 rounded-lg transition-colors ${$audioStore.isMuted ? 'bg-red-500' : 'bg-gray-700'}`}
            on:click={() => audioStore.toggleMute()}
        >
            {$audioStore.isMuted ? 'Unmute' : 'Mute'}
        </button>
        
        <button 
            class={`flex-1 py-2 px-4 rounded-lg transition-colors ${$audioStore.isDeafened ? 'bg-red-500' : 'bg-gray-700'}`}
            on:click={() => audioStore.toggleDeafen()}
        >
            {$audioStore.isDeafened ? 'Undeafen' : 'Deafen'}
        </button>
    </div>
</div>