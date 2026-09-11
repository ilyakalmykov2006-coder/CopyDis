const VOLUMES = "hush.peer-volumes";
export const peerVolumes = { get: (): Record<string, number> => JSON.parse(localStorage.getItem(VOLUMES) ?? "{}") as Record<string, number>, set: (id: string, value: number) => { const all = peerVolumes.get(); all[id] = value; localStorage.setItem(VOLUMES, JSON.stringify(all)); } };
