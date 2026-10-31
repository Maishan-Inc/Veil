import type { MessageKey } from "@veil/i18n";
import { useT } from "@veil/i18n";
import type { Profile } from "@veil/shared";
import { profileStatusTone } from "@veil/shared";
import { Badge } from "@veil/ui";

const STATUS_LABEL: Record<Profile["status"], MessageKey> = {
  idle: "profiles.status.idle",
  launching: "profiles.status.launching",
  running: "profiles.status.running",
  error: "profiles.status.error",
};

export function StatusBadge({ status }: { status: Profile["status"] }) {
  const t = useT();
  const tone = profileStatusTone(status);
  // The shared helper returns "neutral" | "info" | "success" | "error", matching the Badge tone API.
  return (
    <Badge tone={tone} dot pulse={status === "launching"}>
      {t(STATUS_LABEL[status])}
    </Badge>
  );
}
