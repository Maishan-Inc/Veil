import { useT } from "@veil/i18n";
import { Banner, Button } from "@veil/ui";

interface EngineMissingBannerProps {
  onOpenBrowser: () => void;
}

/** Non-blocking banner shown at the top of Profiles while no engine is installed. */
export function EngineMissingBanner({ onOpenBrowser }: EngineMissingBannerProps) {
  const t = useT();
  return (
    <Banner
      tone="warning"
      title={t("profiles.banner.engineMissing.title")}
      action={
        <Button variant="primary" size="sm" onClick={onOpenBrowser}>
          {t("profiles.banner.engineMissing.action")}
        </Button>
      }
    >
      {t("profiles.banner.engineMissing.body")}
    </Banner>
  );
}
