import { create, StoreApi, UseBoundStore } from "zustand";

export interface AppState {
  message: string
}

export type ZustandStore<T> = UseBoundStore<StoreApi<T>>;

export function createStore(initialState: AppState = defaultAppState()): ZustandStore<AppState> {
  return create<AppState>(() => initialState);
}

export function defaultAppState(): AppState {
  return {
    message: "Hello, world!",
  };
}


