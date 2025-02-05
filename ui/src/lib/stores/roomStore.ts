// ui/src/lib/stores/roomStore.ts
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';  // Fixed type-only import
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
    isLoading: boolean;
}

interface RoomEventPayload {
    room_id: string;
    action: string;
    participants: User[];
}

interface ErrorEventPayload {
    code: string;
    message: string;
}

const initialState: RoomState = {
    rooms: [],
    currentRoom: null,
    error: null,
    isLoading: false,
};

function createRoomStore() {
    const { subscribe, update } = writable<RoomState>(initialState);
    let eventUnsubscribers: UnlistenFn[] = [];

    const preserveParticipants = (newRooms: Room[], oldRooms: Room[]): Room[] => {
        return newRooms.map(newRoom => {
            const oldRoom = oldRooms.find(r => r.id === newRoom.id);
            // Fixed undefined checks
            if (oldRoom && oldRoom.participants && oldRoom.participants.length > 0) {
                return newRoom.participants.length === 0 ? 
                    { ...newRoom, participants: oldRoom.participants } : 
                    newRoom;
            }
            return newRoom;
        });
    };

    // Rest of the code remains the same...
    const filterEmptyRooms = (rooms: Room[]): Room[] => {
        return rooms.filter(room => room.participants.length > 0);
    };

    const handleRoomEvent = (state: RoomState, { room_id, action, participants }: RoomEventPayload): RoomState => {
        switch (action) {
            case 'join': {
                const updatedRooms = state.rooms.map(room => 
                    room.id === room_id 
                        ? { ...room, participants }
                        : room
                );
                return {
                    ...state,
                    rooms: updatedRooms,
                    currentRoom: state.currentRoom?.id === room_id 
                        ? { ...state.currentRoom, participants }
                        : state.currentRoom
                };
            }
            case 'leave': {
                const roomsAfterLeave = state.rooms
                    .map(room => 
                        room.id === room_id 
                            ? { ...room, participants }
                            : room
                    )
                    .filter(room => room.participants.length > 0);
                
                return {
                    ...state,
                    rooms: roomsAfterLeave,
                    currentRoom: state.currentRoom?.id === room_id 
                        ? null 
                        : state.currentRoom
                };
            }
            default:
                return state;
        }
    };

    const setupEventListeners = async () => {
        try {
            const roomUnsubscribe = await listen<RoomEventPayload>('room:update', (event) => {
                update(state => handleRoomEvent(state, event.payload));
            });
            eventUnsubscribers.push(roomUnsubscribe);

            const errorUnsubscribe = await listen<ErrorEventPayload>('error', (event) => {
                update(state => ({
                    ...state,
                    error: `${event.payload.code}: ${event.payload.message}`
                }));
            });
            eventUnsubscribers.push(errorUnsubscribe);
        } catch (error) {
            console.error('Failed to setup event listeners:', error);
            throw error;
        }
    };

    const cleanupEventListeners = () => {
        eventUnsubscribers.forEach(unsub => unsub());
        eventUnsubscribers = [];
    };

    return {
        subscribe,
        
        async initialize() {
            cleanupEventListeners();
            await setupEventListeners();
        },

        async refreshRooms() {
            update(state => ({ ...state, isLoading: true }));
            try {
                const rooms = await invoke<Room[]>('list_rooms');
                update(state => {
                    const updatedRooms = preserveParticipants(rooms, state.rooms);
                    const currentRoomId = state.currentRoom?.id;
                    const updatedCurrentRoom = currentRoomId ? 
                        updatedRooms.find(r => r.id === currentRoomId) || null : 
                        null;

                    return {
                        ...state,
                        rooms: filterEmptyRooms(updatedRooms),
                        currentRoom: updatedCurrentRoom,
                        error: null,
                        isLoading: false,
                    };
                });
            } catch (err) {
                update(state => ({ 
                    ...state, 
                    error: err instanceof Error ? err.message : 'Failed to refresh rooms',
                    isLoading: false 
                }));
            }
        },

        async createRoom(name: string, userId: string) {
            update(state => ({ ...state, isLoading: true, error: null }));
            try {
                const currentState = get({ subscribe });
                if (currentState.currentRoom) {
                    await invoke('leave_room', { 
                        roomId: currentState.currentRoom.id, 
                        userId 
                    });
                }

                const room = await invoke<Room>('create_room', { name, userId });
                const currentUser = get(userStore).currentUser;
                if (!currentUser) throw new Error('No current user');
                
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
                
                update(state => ({
                    ...state,
                    rooms: [...filterEmptyRooms(state.rooms), updatedRoom],
                    currentRoom: updatedRoom,
                    error: null,
                    isLoading: false,
                }));
                
                return updatedRoom;
            } catch (err) {
                const errorMessage = err instanceof Error ? err.message : 'Failed to create room';
                update(state => ({
                    ...state,
                    error: errorMessage,
                    isLoading: false,
                }));
                throw new Error(errorMessage);
            }
        },

        async joinRoom(roomId: string, userId: string) {
          console.log('Attempting to join room:', { roomId, userId });
          update(state => ({ ...state, isLoading: true, error: null }));
          try {
              const currentState = get({ subscribe });
              if (currentState.currentRoom) {
                  console.log('Leaving current room before joining new one');
                  await invoke('leave_room', { 
                      roomId: currentState.currentRoom.id, 
                      userId 
                  });
              }
      
              console.log('Invoking join_room command');
              const result = await invoke<Room>('join_room', { roomId, userId });
              console.log('Join room command result:', result);
      
              const currentUser = get(userStore).currentUser;
              if (!currentUser) {
                  throw new Error('No current user found when trying to join room');
              }
      
              update(state => {
                  console.log('Updating store with new room state');
                  const updatedRooms = state.rooms.map(r => r.id === roomId ? {
                      ...r,
                      participants: [
                          ...r.participants.filter(p => p.id !== userId),
                          {
                              id: userId,
                              name: currentUser.name,
                              is_muted: false,
                              is_deafened: false,
                              volume: 1
                          }
                      ]
                  } : r);
      
                  const targetRoom = updatedRooms.find(r => r.id === roomId);
                  if (!targetRoom) {
                      console.warn('Target room not found after update');
                  }
                  return {
                      ...state,
                      rooms: filterEmptyRooms(updatedRooms),
                      currentRoom: targetRoom || null,
                      error: null,
                      isLoading: false,
                  };
              });
          } catch (err) {
              console.error('Failed to join room:', err);
              const errorMessage = err instanceof Error ? err.message : 'Failed to join room';
              update(state => ({
                  ...state,
                  error: errorMessage,
                  isLoading: false,
              }));
              throw new Error(errorMessage);
          }
      },

        async leaveRoom(roomId: string, userId: string) {
            update(state => ({ ...state, isLoading: true, error: null }));
            try {
                await invoke('leave_room', { roomId, userId });
                
                update(state => ({
                    ...state,
                    rooms: filterEmptyRooms(
                        state.rooms.map(room => ({
                            ...room,
                            participants: room.participants.filter(p => p.id !== userId)
                        }))
                    ),
                    currentRoom: null,
                    error: null,
                    isLoading: false,
                }));
            } catch (err) {
                const errorMessage = err instanceof Error ? err.message : 'Failed to leave room';
                update(state => ({
                    ...state,
                    error: errorMessage,
                    isLoading: false,
                }));
                throw new Error(errorMessage);
            }
        },

        clearError() {
            update(state => ({ ...state, error: null }));
        }
    };
}

export const roomStore = createRoomStore();