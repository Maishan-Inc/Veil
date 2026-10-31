import type { ReactNode } from "react";
import { useT } from "@veil/i18n";
import { IconButton, cx } from "@veil/ui";
import { useAppStore, type Section } from "../app/store";
import { Icon, type IconName } from "./Icon";

interface NavItem {
  section: Section;
  label: string;
  icon: IconName;
}

export function Sidebar(): ReactNode {
  const t = useT();
  const { section, sidebarCollapsed, setSection, toggleSidebar } = useAppStore();

  const items: NavItem[] = [
    { section: "profiles", label: t("nav.profiles"), icon: "profiles" },
    { section: "kernel", label: t("nav.kernel"), icon: "browser" },
    { section: "proxy", label: t("nav.proxy"), icon: "proxy" },
    { section: "ai", label: t("nav.ai"), icon: "ai" },
    { section: "extensions", label: t("nav.extensions"), icon: "extensions" },
    { section: "api", label: t("nav.api"), icon: "api" },
    { section: "settings", label: t("nav.settings"), icon: "settings" },
    { section: "logs", label: t("nav.logs"), icon: "logs" },
  ];

  return (
    <nav className="veil-sidebar" aria-label={t("nav.profiles")}>
      <div className="veil-sidebar__group-label" aria-hidden={sidebarCollapsed}>
        {t("app.name")}
      </div>
      {items.map((item) => {
        const active = section === item.section;
        return (
          <button
            key={item.section}
            type="button"
            className="veil-sidebar__item"
            data-active={active}
            aria-current={active ? "page" : undefined}
            onClick={() => setSection(item.section)}
            title={sidebarCollapsed ? item.label : undefined}
          >
            <span className="veil-sidebar__item-icon">
              <Icon name={item.icon} size={18} />
            </span>
            <span className="veil-sidebar__item-label" aria-hidden={sidebarCollapsed}>
              {item.label}
            </span>
          </button>
        );
      })}
      <div className={cx("veil-sidebar__toggle")}>
        <IconButton
          size="sm"
          label={sidebarCollapsed ? t("nav.expand") : t("nav.collapse")}
          onClick={toggleSidebar}
        >
          <Icon name={sidebarCollapsed ? "panelLeft" : "panelLeftClose"} size={16} />
        </IconButton>
      </div>
    </nav>
  );
}
