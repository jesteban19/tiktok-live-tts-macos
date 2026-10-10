import { create } from 'zustand';
import type { Snapshot } from '../types';
export function newer(current:Snapshot|null,incoming:Snapshot):boolean {return !current||incoming.revision>=current.revision;}
export const useApp=create<{state:Snapshot|null;error:string;accept:(s:Snapshot)=>void;setError:(e:string)=>void}>((set)=>({state:null,error:'',accept:(s)=>set((old)=>newer(old.state,s)?{state:s}:{}),setError:(error)=>set({error})}));
