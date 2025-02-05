<!-- ui/src/lib/components/EventTester.svelte -->
<script lang="ts">
    import { onMount } from 'svelte';
    import { get } from 'svelte/store';
    import { invoke } from '@tauri-apps/api/core';
    import { roomStore } from '../stores/roomStore';
    import { userStore } from '../stores/userStore';

    // Don't generate random ID, let the backend handle it
    let testUser = {
        name: 'Test User'
    };

    let testRoom = {
        name: 'Test Room'
    };

    let status = '';
    let error = '';

    onMount(async () => {
        try {
            status = 'Initializing event test component';
            console.log(status);
            await roomStore.initialize();
            status = 'Event listeners initialized successfully';
            console.log(status);
        } catch (err) {
            error = err instanceof Error ? err.message : 'Unknown error';
            console.error('Failed to initialize event listeners:', err);
        }
    });

    async function testCreateAndJoin() {
        try {
            error = '';
            status = 'Creating user...';
            console.log('Testing room creation and joining...');
            
            // Create user and get the returned user object
            const createdUser = await userStore.setUser(testUser.name);
            console.log('Test user created:', createdUser);

            if (!createdUser) {
                throw new Error('Failed to create user');
            }

            // Create room using the ID from the created user
            status = 'Creating room...';
            const room = await roomStore.createRoom(testRoom.name, createdUser.id);
            console.log('Test room created:', room);

            // Join room using the same user ID
            status = 'Joining room...';
            await roomStore.joinRoom(room.id, createdUser.id);
            status = 'Successfully joined room';
            console.log('Joined test room');
        } catch (err) {
            error = err instanceof Error ? err.message : 'Unknown error';
            console.error('Test failed:', err);
            status = 'Test failed';
        }
    }
</script>

<div class="p-4 bg-zinc-800 rounded-lg">
    <h2 class="text-lg font-semibold text-white mb-4">Event Tester</h2>
    
    <div class="mb-4">
        <p class="text-sm text-zinc-300">Status: {status}</p>
        {#if error}
            <p class="text-sm text-red-400 mt-2">{error}</p>
        {/if}
    </div>
    
    <button
        class="px-4 py-2 bg-[#3cf281] text-zinc-900 rounded-lg"
        on:click={testCreateAndJoin}
        disabled={$roomStore.isLoading}
    >
        {$roomStore.isLoading ? 'Testing...' : 'Test Room Events'}
    </button>

    <div class="mt-4 space-y-2">
        {#if $userStore.currentUser}
            <p class="text-sm text-zinc-300">Current User: {$userStore.currentUser.name} (ID: {$userStore.currentUser.id})</p>
        {/if}
        {#if $roomStore.currentRoom}
            <p class="text-sm text-zinc-300">Current Room: {$roomStore.currentRoom.name}</p>
        {/if}
        {#if $roomStore.error}
            <p class="text-sm text-red-400">{$roomStore.error}</p>
        {/if}
    </div>
</div>