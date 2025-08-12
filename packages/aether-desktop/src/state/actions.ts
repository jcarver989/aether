import { commands } from "@/generated/invoke";
import { AppState, ZustandStore } from "./store";

export class AppActions {

  constructor(
    private store: ZustandStore<AppState>,
    private tauriCommands: typeof commands,
  ) { }

  async exampleAction() {
    await this.tauriCommands.exampleCommand();

    this.store.setState((state) => ({
      ...state,
      showOnboarding: false,
    }));
  }


}
