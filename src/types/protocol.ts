export type ConnectionState = "connected" | "reconnecting" | "offline";
export interface ChatMessage { id: string; authorId: string; authorName: string; content: string; createdAt: string; editedAt: string | null; }
export interface RoomUser { id: string; name: string; inVoice: boolean; muted: boolean; speaking: boolean; }
export type ClientEvent =
 | { type: "authenticate"; username: string; password: string }
 | { type: "message:create"; content: string }
 | { type: "message:update"; id: string; content: string }
 | { type: "message:delete"; id: string }
 | { type: "typing"; active: boolean }
 | { type: "voice:state"; joined: boolean; muted: boolean }
 | { type: "signal"; to: string; data: RTCSessionDescriptionInit | RTCIceCandidateInit };
export type ServerEvent =
 | { type: "ready"; userId: string; room: { name: string; maxUsers: number }; messages: ChatMessage[]; users: RoomUser[] }
 | { type: "message:create"; message: ChatMessage }
 | { type: "message:update"; message: ChatMessage }
 | { type: "message:delete"; id: string }
 | { type: "presence"; users: RoomUser[] }
 | { type: "typing"; userId: string; name: string; active: boolean }
 | { type: "signal"; from: string; data: RTCSessionDescriptionInit | RTCIceCandidateInit }
 | { type: "error"; code: string; message: string };
