<!-- ui/src/lib/components/AudioDeviceManager.svelte -->
<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { audioStore } from '../stores/audioStore';
    import AudioMeter from './AudioMeter.svelte';
  
    let stream: MediaStream | null = null;
    let error: string | null = null;
    let selectedInputId: string | null = null;
    let selectedOutputId: string | null = null;
    let inputDevices: MediaDeviceInfo[] = [];
    let outputDevices: MediaDeviceInfo[] = [];

    // Subscribe to audio store
    $: ({ inputVolume, outputVolume, isMuted, isDeafened, error: storeError } = $audioStore);
    $: if (storeError) error = storeError;

    async function loadDevices() {
        try {
            await navigator.mediaDevices.getUserMedia({ audio: true });
            const devices = await navigator.mediaDevices.enumerateDevices();
            
            // Filter audio devices
            inputDevices = devices.filter(device => device.kind === 'audioinput');
            outputDevices = devices.filter(device => device.kind === 'audiooutput');
            
            // Set default device if none selected
            if (!selectedInputId && inputDevices.length > 0) {
                selectedInputId = inputDevices[0].deviceId;
                await handleInputChange({ target: { value: selectedInputId } } as any);
            }
            if (!selectedOutputId && outputDevices.length > 0) {
                selectedOutputId = outputDevices[0].deviceId;
                await handleOutputChange({ target: { value: selectedOutputId } } as any);
            }
        } catch (err) {
            error = err instanceof Error ? err.message : 'Failed to load audio devices';
        }
    }

    async function handleInputChange(event: Event) {
        try {
            const deviceId = (event.target as HTMLSelectElement).value;
            selectedInputId = deviceId;
            
            // Stop existing stream if any
            if (stream) {
                stream.getTracks().forEach(track => track.stop());
            }
            
            // Start new stream for monitoring
            stream = await navigator.mediaDevices.getUserMedia({
                audio: {
                    deviceId: { exact: deviceId },
                    echoCancellation: true,
                    noiseSuppression: true,
                    autoGainControl: true
                }
            });

            // Set the device in CPAL
            await audioStore.setInputDevice(deviceId);
        } catch (err) {
            error = err instanceof Error ? err.message : 'Failed to switch audio input device';
        }
    }

    async function handleOutputChange(event: Event) {
        try {
            const deviceId = (event.target as HTMLSelectElement).value;
            selectedOutputId = deviceId;
            
            // Set output device for HTML audio elements
            if ('setSinkId' in HTMLAudioElement.prototype) {
                const audioElements = document.querySelectorAll('audio');
                await Promise.all(
                    Array.from(audioElements).map(audio => 
                        (audio as any).setSinkId(deviceId)
                    )
                );
            }
        } catch (err) {
            error = err instanceof Error ? err.message : 'Failed to switch audio output device';
        }
    }

    function handleInputVolumeChange(event: Event) {
        const volume = parseFloat((event.target as HTMLInputElement).value);
        audioStore.setInputVolume(volume);
    }

    function handleOutputVolumeChange(event: Event) {
        const volume = parseFloat((event.target as HTMLInputElement).value);
        audioStore.setUserVolume('global', volume);
    }

    function toggleMute() {
        audioStore.toggleMute();
    }

    function toggleDeafen() {
        audioStore.toggleDeafen();
    }

    // Set up device monitoring
    onMount(async () => {
        await loadDevices();
        navigator.mediaDevices.addEventListener('devicechange', loadDevices);
    });

    onDestroy(() => {
        if (stream) {
            stream.getTracks().forEach(track => track.stop());
        }
        navigator.mediaDevices.removeEventListener('devicechange', loadDevices);
    });
</script>

<div class="p-4 bg-zinc-800 rounded-lg shadow-lg">
    <h2 class="text-xl font-semibold text-white mb-4">Audio Settings</h2>
    
    <!-- Input Device Selection -->
    <div class="mb-4">
        <label for="input-device" class="block text-sm font-medium text-zinc-300 mb-2">
            Microphone
        </label>
        <select
            id="input-device"
            class="w-full bg-zinc-700 text-white rounded-md px-3 py-2"
            on:change={handleInputChange}
            bind:value={selectedInputId}
        >
            {#each inputDevices as device}
                <option value={device.deviceId}>
                    {device.label || `Microphone ${device.deviceId}`}
                </option>
            {/each}
        </select>
    </div>

    <!-- Output Device Selection -->
    <div class="mb-4">
        <label for="output-device" class="block text-sm font-medium text-zinc-300 mb-2">
            Speaker
        </label>
        <select
            id="output-device"
            class="w-full bg-zinc-700 text-white rounded-md px-3 py-2"
            on:change={handleOutputChange}
            bind:value={selectedOutputId}
        >
            {#each outputDevices as device}
                <option value={device.deviceId}>
                    {device.label || `Speaker ${device.deviceId}`}
                </option>
            {/each}
        </select>
    </div>

    <!-- Volume Controls -->
    <div class="space-y-4">
        <div>
            <label for="input-volume" class="block text-sm font-medium text-zinc-300 mb-2">
                Input Volume
            </label>
            <input
                type="range"
                id="input-volume"
                min="0"
                max="1"
                step="0.01"
                class="w-full"
                on:input={handleInputVolumeChange}
                value={inputVolume}
            />
        </div>

        <div>
            <label for="output-volume" class="block text-sm font-medium text-zinc-300 mb-2">
                Output Volume
            </label>
            <input
                type="range"
                id="output-volume"
                min="0"
                max="1"
                step="0.01"
                class="w-full"
                on:input={handleOutputVolumeChange}
                value={outputVolume}
            />
        </div>
    </div>

    <!-- Audio Controls -->
    <div class="flex gap-4 mt-4">
        <button
            class="px-4 py-2 rounded-md bg-zinc-700 text-white hover:bg-zinc-600 transition-colors"
            class:bg-red-600={isMuted}
            on:click={toggleMute}
        >
            {isMuted ? 'Unmute' : 'Mute'}
        </button>

        <button
            class="px-4 py-2 rounded-md bg-zinc-700 text-white hover:bg-zinc-600 transition-colors"
            class:bg-red-600={isDeafened}
            on:click={toggleDeafen}
        >
            {isDeafened ? 'Undeafen' : 'Deafen'}
        </button>
    </div>

    <!-- Audio Meter -->
    {#if stream}
        <div class="mt-4">
            <AudioMeter {stream} />
        </div>
    {/if}

    <!-- Error Display -->
    {#if error}
        <div class="mt-4 p-3 bg-red-500 text-white rounded-md">
            {error}
        </div>
    {/if}
</div>