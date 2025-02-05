// ui/src/lib/stores/userStore.ts
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { User } from '../types/user';

export interface UserState {
    currentUser: User | null;
    error: string | null;
    isLoading: boolean;  // Add loading state
}

const initialState: UserState = {
    currentUser: null,
    error: null,
    isLoading: false
};

function createUserStore() {
    const { subscribe, update } = writable<UserState>(initialState);

    return {
        subscribe,

        async setUser(name: string): Promise<User> {
            update(state => ({ ...state, isLoading: true, error: null }));
            try {
                // Call the backend to add the user
                const user = await invoke<User>('add_user', { name });
                
                if (!user) {
                    throw new Error('Failed to create user - no user returned from server');
                }

                console.log('User created successfully:', user);

                update(state => ({
                    ...state,
                    currentUser: user,
                    error: null,
                    isLoading: false
                }));

                return user;  // Return the user for chaining
            } catch (err) {
                console.error('Failed to set user:', err);
                const errorMessage = err instanceof Error ? err.message : 'Failed to set user';
                update(state => ({
                    ...state,
                    error: errorMessage,
                    isLoading: false
                }));
                throw new Error(errorMessage);
            }
        },

        async updateName(name: string) {
            const currentUser = get({ subscribe }).currentUser;
            if (!currentUser) {
                console.error('Cannot update name: no current user');
                return;
            }

            try {
                // You might want to add a backend call here if you need to persist the name change
                update(state => ({
                    ...state,
                    currentUser: state.currentUser ? { ...state.currentUser, name } : null
                }));
            } catch (err) {
                console.error('Failed to update name:', err);
                throw err;
            }
        },

        async setMuted(is_muted: boolean) {
            const currentUser = get({ subscribe }).currentUser;
            if (!currentUser) {
                console.error('Cannot set mute state: no current user');
                return;
            }

            try {
                await invoke('set_muted', { muted: is_muted });
                update(state => ({
                    ...state,
                    currentUser: state.currentUser ? { ...state.currentUser, is_muted } : null
                }));
            } catch (err) {
                console.error('Failed to set mute state:', err);
                throw err;
            }
        },

        async setDeafened(is_deafened: boolean) {
            const currentUser = get({ subscribe }).currentUser;
            if (!currentUser) {
                console.error('Cannot set deafened state: no current user');
                return;
            }

            try {
                // You might want to add a backend call here if you need to persist the deafened state
                update(state => ({
                    ...state,
                    currentUser: state.currentUser ? { ...state.currentUser, is_deafened } : null
                }));
            } catch (err) {
                console.error('Failed to set deafened state:', err);
                throw err;
            }
        },

        async setVolume(volume: number) {
            const currentUser = get({ subscribe }).currentUser;
            if (!currentUser) {
                console.error('Cannot set volume: no current user');
                return;
            }

            try {
                await invoke('set_user_volume', { volume });
                update(state => ({
                    ...state,
                    currentUser: state.currentUser ? { ...state.currentUser, volume } : null
                }));
            } catch (err) {
                console.error('Failed to set volume:', err);
                throw err;
            }
        },

        clearUser() {
            update(state => ({
                ...state,
                currentUser: null,
                error: null
            }));
        },

        clearError() {
            update(state => ({ ...state, error: null }));
        }
    };
}

export const userStore = createUserStore();