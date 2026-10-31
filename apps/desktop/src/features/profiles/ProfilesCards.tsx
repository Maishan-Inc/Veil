import type { ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { AppErrorCode, Profile } from "@veil/shared";
import { Checkbox, IconButton } from "@veil/ui";
import { Icon } from "../../shell/Icon";
import { useAppStore } from "../../app/store";
import { formatLastLaunched, osLabel } from "./format";
import type { LaunchDiagnostics } from "./launchDiagnostics";
import { StatusBadge } from "./StatusBadge";

export interface ProfilesCardsProps {
  profiles: Profile[];
  /** id → display name for the proxy line. */
  proxyNames: Map<string, string>;
  /** id → launch/stop error code surfaced by the backend. */
  launchErrors: Record<string, AppErrorCode | null | undefined>;
  /** id → launch diagnostics computed from profile, runtime, and proxy state. */
  launchDiagnostics: Record<string, LaunchDiagnostics | undefined>;
  onLaunch: (profile: Profile) => void;
  onStop: (profile: Profile) => void;
  onEdit: (profile: Profile) => void;
  onModifyProxy: (profile: Profile) => void;
  onDelete: (profile: Profile) => void;
}

/** Card presentation of the profile list — the Table View's data flow, re-laid out. */
export function ProfilesCards({
  profiles,
  proxyNames,
  launchErrors,
  launchDiagnostics,
  onLaunch,
  onStop,
  onEdit,
  onModifyProxy,
  onDelete,
}: ProfilesCardsProps): ReactNode {
  const t = useT();
  const { selectedProfileIds, toggleSelection } = useAppStore();

  return (
    <div className="profiles-cards" role="list" aria-rowcount={profiles.length}>
      {profiles.map((profile) => {
        const selected = selectedProfileIds.has(profile.id);
        const launchError = launchErrors[profile.id] ?? null;
        const diagnostics = launchDiagnostics[profile.id];
        const proxyLabel = profile.proxyId
          ? (proxyNames.get(profile.proxyId) ?? profile.proxyId)
          : t("profiles.noProxy");
        return (
          <div
            key={profile.id}
            className="profile-card"
            role="listitem"
            data-selected={selected}
            data-runtime-error={Boolean(launchError)}
          >
            <header className="profile-card__head">
              <Checkbox
                aria-label={profile.name}
                checked={selected}
                onChange={() => toggleSelection(profile.id)}
              />
              <span
                className="profile-card__avatar"
                style={{ background: profile.avatarColor }}
                aria-hidden="true"
              >
                {profile.name.slice(0, 1).toUpperCase()}
              </span>
              <span className="profile-card__name" title={profile.name}>
                {profile.name}
              </span>
              <StatusBadge status={profile.status} />
            </header>
            <dl className="profile-card__meta">
              <div className="profile-card__meta-row">
                <dt>{t("profiles.column.browserVersion")}</dt>
                <dd>{profile.browserVersion || diagnostics?.kernelVersion || "—"}</dd>
              </div>
              <div className="profile-card__meta-row">
                <dt>{t("profiles.column.proxy")}</dt>
                <dd>
                  <button
                    type="button"
                    className="profile-card__proxy"
                    title={t("profiles.batch.modifyProxy")}
                    onClick={() => onModifyProxy(profile)}
                  >
                    {proxyLabel}
                    {diagnostics?.proxyHint ? (
                      <span className="profile-card__proxy-hint">
                        {t(
                          diagnostics.proxyHint === "authIgnored"
                            ? "profiles.diagnostics.proxyAuthIgnored"
                            : "profiles.diagnostics.proxyMissing",
                        )}
                      </span>
                    ) : null}
                  </button>
                </dd>
              </div>
              <div className="profile-card__meta-row">
                <dt>{t("profiles.column.os")}</dt>
                <dd>{t(osLabel(profile.os))}</dd>
              </div>
              <div className="profile-card__meta-row">
                <dt>{t("profiles.column.platform")}</dt>
                <dd>{profile.platform || "—"}</dd>
              </div>
              <div className="profile-card__meta-row">
                <dt>{t("profiles.column.lastLaunched")}</dt>
                <dd>{formatLastLaunched(profile.lastLaunchedAt, t("profiles.neverLaunched"))}</dd>
              </div>
              {profile.notes ? (
                <div className="profile-card__meta-row profile-card__meta-row--notes">
                  <dt>{t("profiles.column.notes")}</dt>
                  <dd title={profile.notes}>{profile.notes}</dd>
                </div>
              ) : null}
            </dl>
            <footer className="profile-card__actions">
              {profile.status === "running" || profile.status === "launching" ? (
                <IconButton size="sm" label={t("common.stop")} onClick={() => onStop(profile)}>
                  <Icon name="stop" size={14} />
                </IconButton>
              ) : (
                <IconButton
                  size="sm"
                  label={t("profiles.launch")}
                  onClick={() => onLaunch(profile)}
                >
                  <Icon name="play" size={14} />
                </IconButton>
              )}
              <IconButton size="sm" label={t("common.edit")} onClick={() => onEdit(profile)}>
                <Icon name="edit" size={14} />
              </IconButton>
              <IconButton size="sm" label={t("common.delete")} onClick={() => onDelete(profile)}>
                <Icon name="trash" size={14} />
              </IconButton>
            </footer>
          </div>
        );
      })}
    </div>
  );
}
