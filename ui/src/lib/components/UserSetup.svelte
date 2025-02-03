<!-- ui/src/lib/components/UserSetup.svelte -->
<script lang="ts">
    import { userStore } from '../stores/userStore';
    import { fade } from 'svelte/transition';

    let name = '';
    let error = '';

    async function handleSubmit() {
        if (!name.trim()) {
            error = 'Please enter a name';
            return;
        }

        try {
            await userStore.setUser(name.trim());
        } catch (err) {
            error = err instanceof Error ? err.message : 'Failed to set user name';
        }
    }
</script>

<div
    class="fixed inset-0 bg-black/50 flex items-center justify-center p-4 z-50"
    transition:fade
>
    <div class="bg-zinc-800 rounded-lg shadow-xl p-6 w-full max-w-md">
        <h2 class="text-xl font-semibold text-white mb-4">Welcome to LLAS</h2>
        
        <p class="text-zinc-300 mb-6">
            Please enter your name to get started.
        </p>

        <form on:submit|preventDefault={handleSubmit} class="space-y-4">
            <div>
                <label for="name" class="block text-sm font-medium text-zinc-300 mb-1">
                    Display Name
                </label>
                <input
                    type="text"
                    id="name"
                    bind:value={name}
                    placeholder="Enter your name"
                    class="w-full px-3 py-2 bg-zinc-700 border border-zinc-600 rounded-md text-white placeholder-zinc-400 focus:outline-none focus:ring-2 focus:ring-[#3cf281]"
                />
                {#if error}
                    <p class="mt-1 text-sm text-red-400">{error}</p>
                {/if}
            </div>

            <button
                type="submit"
                class="w-full py-2 px-4 bg-[#3cf281] hover:bg-[#34d973] text-zinc-900 font-medium rounded-md transition-colors"
            >
                Join
            </button>
        </form>
    </div>
</div>
