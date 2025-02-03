// ui/src/lib/stores/roomStore.ts
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { userStore } from './userStore';
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

  // Helper to preserve participant state when updating rooms
  const preserveParticipants = (newRooms: Room[], oldRooms: Room[]): Room[] => {
    return newRooms.map(newRoom => {
      const oldRoom = oldRooms.find(r => r.id === newRoom.id);
      if (oldRoom && oldRoom.participants.length > 0) {
        // Keep old participants if the new room has none
        return newRoom.participants.length === 0 ? 
          { ...newRoom, participants: oldRoom.participants } : 
          newRoom;
      }
      return newRoom;
    });
  };

  // Helper to filter out empty rooms
  const filterEmptyRooms = (rooms: Room[]): Room[] => {
    return rooms.filter(room => room.participants.length > 0);
  };

  return {
    subscribe,

    refreshRooms: async () => {
      try {
        const rooms = await invoke<Room[]>('list_rooms');
        
        update(state => {
          // Preserve participant state when updating rooms
          const updatedRooms = preserveParticipants(rooms, state.rooms);
          
          // Update current room with latest data if we're in one
          const currentRoomId = state.currentRoom?.id;
          const updatedCurrentRoom = currentRoomId ? 
            updatedRooms.find(r => r.id === currentRoomId) || null : 
            null;

          // Filter out empty rooms
          const activeRooms = filterEmptyRooms(updatedRooms);

          return {
            ...state,
            rooms: activeRooms,
            currentRoom: updatedCurrentRoom,
            error: null,
          };
        });
      } catch (err) {
        console.error('Failed to refresh rooms:', err);
      }
    },

    createRoom: async (name: string, userId: string) => {
      try {
        // If user is in a room, leave it first
        const state = get({ subscribe });
        if (state.currentRoom) {
          await invoke('leave_room', { roomId: state.currentRoom.id, userId });
        }

        const room = await invoke<Room>('create_room', { name, userId });
        
        // Get current user's name
        const currentUser = get(userStore).currentUser;
        if (!currentUser) throw new Error('No current user');
        
        // Add the creator as a participant with their name
        const updatedRoom = {
          ...room,
          participants: [{
            id: userId,
            name: currentUser.name,
            is_muted: false,
            is_deafened: false,
            volume: 1
          }],
        };
        
        update(state => {
          // Remove user from any other rooms they were in
          const updatedRooms = state.rooms.map(r => ({
            ...r,
            participants: r.participants.filter(p => p.id !== userId)
          }));

          // Filter out empty rooms
          const activeRooms = filterEmptyRooms(updatedRooms);
          
          return {
            ...state,
            rooms: [...activeRooms, updatedRoom],
            currentRoom: updatedRoom,
            error: null,
          };
        });
        return updatedRoom;
      } catch (err) {
        console.error('Failed to create room:', err);
        update(state => ({
          ...state,
          error: err instanceof Error ? err.message : 'Failed to create room',
        }));
        throw err;
      }
    },

    joinRoom: async (roomId: string, userId: string) => {
      try {
        // If user is in a room, leave it first
        const state = get({ subscribe });
        if (state.currentRoom) {
          await invoke('leave_room', { roomId: state.currentRoom.id, userId });
        }

        const room = await invoke<Room>('join_room', { roomId, userId });
        
        update(state => {
          // Get current user's name
          const currentUser = get(userStore).currentUser;
          if (!currentUser) throw new Error('No current user');
          
          // Remove user from any other rooms they were in
          const updatedRooms = state.rooms.map(r => ({
            ...r,
            participants: r.participants.filter(p => p.id !== userId)
          }));

          // Add user to the new room
          const targetRoom = updatedRooms.find(r => r.id === roomId);
          if (targetRoom) {
            targetRoom.participants.push({
              id: userId,
              name: currentUser.name,
              is_muted: false,
              is_deafened: false,
              volume: 1
            });
          }

          // Filter out empty rooms
          const activeRooms = filterEmptyRooms(updatedRooms);
          
          return {
            ...state,
            rooms: activeRooms,
            currentRoom: targetRoom || null,
            error: null,
          };
        });
      } catch (err) {
        console.error('Failed to join room:', err);
        update(state => ({
          ...state,
          error: err instanceof Error ? err.message : 'Failed to join room',
        }));
        throw err;
      }
    },

    leaveRoom: async (roomId: string, userId: string) => {
      try {
        await invoke('leave_room', { roomId, userId });
        
        // Remove the user from participants
        update(state => {
          const updatedRooms = state.rooms.map(room => {
            if (room.id === roomId) {
              return {
                ...room,
                participants: room.participants.filter(p => p.id !== userId),
              };
            }
            return room;
          });
          
          // Filter out empty rooms
          const activeRooms = filterEmptyRooms(updatedRooms);

          return {
            ...state,
            rooms: activeRooms,
            currentRoom: null,
            error: null,
          };
        });
      } catch (err) {
        console.error('Failed to leave room:', err);
        update(state => ({
          ...state,
          error: err instanceof Error ? err.message : 'Failed to leave room',
        }));
        throw err;
      }
    },
  };
}

export const roomStore = createRoomStore();