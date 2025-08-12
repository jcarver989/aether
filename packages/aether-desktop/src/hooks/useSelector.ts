import { AppState } from "@/state/store";
import { useAppContext } from "./useAppContext";
import { useStore } from "zustand";

export function useSelector<T>(selector: (state: AppState) => T): T {
  const { store } = useAppContext();
  return useStore(store, selector);
}
