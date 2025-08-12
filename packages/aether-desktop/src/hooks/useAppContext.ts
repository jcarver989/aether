import { commands } from "@/generated/invoke";
import { AppState, ZustandStore } from "@/state/store";
import { Channel } from "@tauri-apps/api/core";
import { createContext, useContext } from "react";
import { AppActions } from "@/state/actions";

export interface AppContext {
  commands: typeof commands;
  createChannel: <T>(onMessage?: (message: T) => void) => Channel<T>;
  store: ZustandStore<AppState>;
  actions: AppActions;
}

export const AppContext = createContext<AppContext | null>(null);

export function useAppContext(): AppContext {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error("useAppContext must be used within an AppContextProvider");
  }

  return context;
}
