import { useSelector } from "@/hooks/useSelector";

export function exampleSelector(): string {
  const message = useSelector((_) => _.message);
  return message;
}

