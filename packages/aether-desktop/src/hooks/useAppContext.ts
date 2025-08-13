import type { AppState, ZustandStore } from "../state/store";
import type { ChannelFactory } from "../types/ui";
import { createContext, useContext } from "react";
import { AppActions } from "../state/actions";

export interface AppContext {
  store: ZustandStore<AppState>;
  actions: AppActions;
  createChannel: ChannelFactory;
}

export const AppContext = createContext<AppContext | null>(null);

export function useAppContext(): AppContext {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error("useAppContext must be used within an AppContextProvider");
  }

  return context;
}
