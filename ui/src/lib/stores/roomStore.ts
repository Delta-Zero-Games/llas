// ui/src/lib/stores/roomStore.ts
import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { User } from '../types/user';

export interface Room {
  id: string;
  name: string;
  creator_id: string;
  participants: User[];
  created_at: string;
}

export interface RoomState {
  rooms: Room[];
  currentRoom: Room | null;
  error: string | null;
}

const initialState: RoomState = {
  rooms: [],
  currentRoom: null,
  error: null,
};

function createRoomStore() {
  const { subscribe, set, update } = writable<RoomState>(initialState);

  return {
    subscribe,
    createRoom: async (name: string, userId: string) => {
      try {
        const room = await invoke<Room>('create_room', { name, userId });
        update(state => ({
          ...state,
          rooms: [...state.rooms, room],
          currentRoom: room,
        }));
      } catch (err) {
        update(state => ({
          ...state,
          error: err instanceof Error ? err.message : 'Failed to create room',
        }));
      }
    },
    joinRoom: async (roomId: string, userId: string) => {
        try {
            const room = await invoke<Room>('join_room', { roomId, userId });
            await invoke('start_streaming', { roomId });
            update(state => ({
                ...state,
                currentRoom: room,
            }));
        } catch (err) {
            update(state => ({
                ...state,
                error: err instanceof Error ? err.message : 'Failed to join room',
            }));
        }
    },
    
    leaveRoom: async (roomId: string, userId: string) => {
        try {
            await invoke('stop_streaming');
            await invoke('leave_room', { roomId, userId });
            update(state => ({
                ...state,
                currentRoom: null,
            }));
        } catch (err) {
            update(state => ({
                ...state,
                error: err instanceof Error ? err.message : 'Failed to leave room',
            }));
        }
    },
    refreshRooms: async () => {
      try {
        const rooms = await invoke<Room[]>('list_rooms');
        update(state => ({
          ...state,
          rooms,
        }));
      } catch (err) {
        update(state => ({
          ...state,
          error: err instanceof Error ? err.message : 'Failed to fetch rooms',
        }));
      }
    },
  };
}

export const roomStore = createRoomStore();