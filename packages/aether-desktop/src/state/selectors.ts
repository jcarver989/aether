import { useSelector } from "@/hooks/useSelector";
import type { ChatMessageBlock, StreamingMessageBlock } from "../types/ui";

export function useMessages(): ChatMessageBlock[] {
  return useSelector((state) => state.messages);
}

export function useStreamingMessage(): StreamingMessageBlock | null {
  return useSelector((state) => state.streamingMessage);
}

export function useSelectedMessageId(): string | null {
  return useSelector((state) => state.selectedMessageId);
}

export function useIsStreaming(): boolean {
  return useSelector((state) => state.streamingMessage !== null);
}

export function useConfig() {
  return useSelector((state) => state.config);
}

export function useConnectionStatus() {
  return useSelector((state) => state.status?.connection_status);
}

export function useAvailableTools() {
  return useSelector((state) => state.status?.available_tools || []);
}

export function useTheme() {
  return useSelector((state) => state.ui.theme);
}

export function useUIState() {
  return useSelector((state) => state.ui);
}

export function useScrollState() {
  return useSelector((state) => state.scroll);
}

