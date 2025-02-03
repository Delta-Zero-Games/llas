// ui/src/lib/stores/userStore.ts
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { User } from '../types/user';

export interface UserState {
    currentUser: User | null;
    error: string | null;
}

const initialState: UserState = {
    currentUser: null,
    error: null
};

function createUserStore() {
    const { subscribe, set, update } = writable<UserState>(initialState);

    return {
        subscribe,

        setUser: async (name: string) => {
            try {
                // Call the backend to add the user
                const user = await invoke<User>('add_user', { name });

                update(state => ({
                    ...state,
                    currentUser: user,
                    error: null
                }));
            } catch (err) {
                console.error('Failed to set user:', err);
                update(state => ({
                    ...state,
                    error: err instanceof Error ? err.message : 'Failed to set user'
                }));
                throw err; // Re-throw to let the component handle the error
            }
        },

        updateName: async (name: string) => {
            const currentUser = get(userStore).currentUser;
            if (!currentUser) return;

            update(state => ({
                ...state,
                currentUser: state.currentUser ? { ...state.currentUser, name } : null
            }));
        },

        setMuted: async (is_muted: boolean) => {
            const currentUser = get(userStore).currentUser;
            if (!currentUser) return;

            try {
                await invoke('set_muted', { muted: is_muted });
                update(state => ({
                    ...state,
                    currentUser: state.currentUser ? { ...state.currentUser, is_muted } : null
                }));
            } catch (err) {
                console.error('Failed to set mute state:', err);
            }
        },

        setDeafened: async (is_deafened: boolean) => {
            const currentUser = get(userStore).currentUser;
            if (!currentUser) return;

            update(state => ({
                ...state,
                currentUser: state.currentUser ? { ...state.currentUser, is_deafened } : null
            }));
        },

        setVolume: async (volume: number) => {
            const currentUser = get(userStore).currentUser;
            if (!currentUser) return;

            try {
                await invoke('set_user_volume', { volume });
                update(state => ({
                    ...state,
                    currentUser: state.currentUser ? { ...state.currentUser, volume } : null
                }));
            } catch (err) {
                console.error('Failed to set volume:', err);
            }
        }
    };
}

export const userStore = createUserStore();
