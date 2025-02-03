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
                const userId = crypto.randomUUID();
                const user: User = {
                    id: userId,
                    name,
                    isOnline: true,
                    status: 'online',
                    is_muted: false,
                    is_deafened: false,
                    volume: 1
                };

                // Notify the backend about the new user
                await invoke('set_user', { user });

                update(state => ({
                    ...state,
                    currentUser: user,
                    error: null
                }));
            } catch (err) {
                update(state => ({
                    ...state,
                    error: err instanceof Error ? err.message : 'Failed to set user'
                }));
            }
        },

        updateStatus: async (status: User['status']) => {
            try {
                const currentState = get(userStore);
                if (!currentState.currentUser) {
                    throw new Error('No user is currently set');
                }

                const updatedUser = {
                    ...currentState.currentUser,
                    status,
                    lastSeen: status === 'offline' ? new Date() : undefined
                };

                // Notify the backend about the status change
                await invoke('update_user_status', { 
                    userId: updatedUser.id, 
                    status 
                });

                update(state => ({
                    ...state,
                    currentUser: updatedUser,
                    error: null
                }));
            } catch (err) {
                update(state => ({
                    ...state,
                    error: err instanceof Error ? err.message : 'Failed to update status'
                }));
            }
        },

        updateName: async (name: string) => {
            try {
                const currentState = get(userStore);
                if (!currentState.currentUser) {
                    throw new Error('No user is currently set');
                }

                const updatedUser = {
                    ...currentState.currentUser,
                    name
                };

                // Notify the backend about the name change
                await invoke('update_user_name', { 
                    userId: updatedUser.id, 
                    name 
                });

                update(state => ({
                    ...state,
                    currentUser: updatedUser,
                    error: null
                }));
            } catch (err) {
                update(state => ({
                    ...state,
                    error: err instanceof Error ? err.message : 'Failed to update name'
                }));
            }
        },

        updateAudioState: async (changes: { is_muted?: boolean; is_deafened?: boolean }) => {
            try {
                const currentState = get(userStore);
                if (!currentState.currentUser) {
                    throw new Error('No user is currently set');
                }

                const updatedUser = {
                    ...currentState.currentUser,
                    ...changes
                };

                // Notify the backend about the audio state change
                await invoke('update_user_audio_state', { 
                    userId: updatedUser.id, 
                    ...changes
                });

                update(state => ({
                    ...state,
                    currentUser: updatedUser,
                    error: null
                }));
            } catch (err) {
                update(state => ({
                    ...state,
                    error: err instanceof Error ? err.message : 'Failed to update audio state'
                }));
            }
        },

        clearError: () => {
            update(state => ({ ...state, error: null }));
        }
    };
}

export const userStore = createUserStore();
