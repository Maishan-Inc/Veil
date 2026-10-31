import type { ReactNode } from "react";

export type IconName =
  | "search"
  | "plus"
  | "import"
  | "export"
  | "refresh"
  | "sun"
  | "moon"
  | "profiles"
  | "browser"
  | "proxy"
  | "ai"
  | "extensions"
  | "api"
  | "settings"
  | "logs"
  | "panelLeft"
  | "panelLeftClose"
  | "play"
  | "stop"
  | "trash"
  | "edit"
  | "eye"
  | "copy"
  | "folder"
  | "moreHorizontal"
  | "check"
  | "minus"
  | "square"
  | "table"
  | "cards"
  | "x";

export interface IconProps {
  name: IconName;
  size?: number;
}

/**
 * Tiny in-tree icon set — original, geometric, stroke-based glyphs styled to feel native to
 * the Fluent-inspired shell. We do not load third-party icon sets to keep the surface offline
 * and avoid copying anyone's marks.
 */
export function Icon({ name, size = 16 }: IconProps): ReactNode {
  const common = {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.6,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };
  switch (name) {
    case "search":
      return (
        <svg {...common}>
          <circle cx="11" cy="11" r="6" />
          <path d="m20 20-3.4-3.4" />
        </svg>
      );
    case "plus":
      return (
        <svg {...common}>
          <path d="M12 5v14M5 12h14" />
        </svg>
      );
    case "import":
      return (
        <svg {...common}>
          <path d="M12 3v12" />
          <path d="m7 10 5 5 5-5" />
          <path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />
        </svg>
      );
    case "export":
      return (
        <svg {...common}>
          <path d="M12 21V9" />
          <path d="m7 14 5-5 5 5" />
          <path d="M4 7V5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2" />
        </svg>
      );
    case "refresh":
      return (
        <svg {...common}>
          <path d="M20 12a8 8 0 1 1-2.34-5.66" />
          <path d="M20 4v5h-5" />
        </svg>
      );
    case "sun":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
        </svg>
      );
    case "moon":
      return (
        <svg {...common}>
          <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z" />
        </svg>
      );
    case "profiles":
      return (
        <svg {...common}>
          <circle cx="9" cy="8" r="3" />
          <path d="M3 21v-1a6 6 0 0 1 12 0v1" />
          <circle cx="17" cy="9" r="2.5" />
          <path d="M14.5 21v-.5a4.5 4.5 0 0 1 7 0v.5" />
        </svg>
      );
    case "browser":
      return (
        <svg {...common}>
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M3 9h18" />
          <circle cx="6.5" cy="6.5" r="0.6" fill="currentColor" />
          <circle cx="9" cy="6.5" r="0.6" fill="currentColor" />
          <circle cx="11.5" cy="6.5" r="0.6" fill="currentColor" />
        </svg>
      );
    case "proxy":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="9" />
          <path d="M3 12h18M12 3a13 13 0 0 1 0 18M12 3a13 13 0 0 0 0 18" />
        </svg>
      );
    case "ai":
      return (
        <svg {...common}>
          <path d="M12 3v3M12 18v3M4.8 7.2l2.1 2.1M17.1 16.8l2.1 2.1M3 12h3M18 12h3M4.8 16.8l2.1-2.1M17.1 7.2l2.1-2.1" />
          <rect x="8" y="8" width="8" height="8" rx="2" />
          <path d="M10.5 12h3M12 10.5v3" />
        </svg>
      );
    case "extensions":
      return (
        <svg {...common}>
          <path d="M8 3h4v3a1.5 1.5 0 1 0 3 0V3h4a1 1 0 0 1 1 1v4h-3a1.5 1.5 0 1 0 0 3h3v4a1 1 0 0 1-1 1h-4v3a1.5 1.5 0 1 1-3 0v-3H8a1 1 0 0 1-1-1v-4h3a1.5 1.5 0 1 0 0-3H7V4a1 1 0 0 1 1-1Z" />
        </svg>
      );
    case "api":
      return (
        <svg {...common}>
          <path d="M8 7 3 12l5 5" />
          <path d="m16 7 5 5-5 5" />
          <path d="m14 4-4 16" />
        </svg>
      );
    case "settings":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.6 1.6 0 0 0 .32 1.77l.05.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.6 1.6 0 0 0-1.77-.31 1.6 1.6 0 0 0-1 1.45V21a2 2 0 1 1-4 0v-.08a1.6 1.6 0 0 0-1-1.45 1.6 1.6 0 0 0-1.77.32l-.06.05a2 2 0 1 1-2.83-2.83l.06-.06A1.6 1.6 0 0 0 4.6 15a1.6 1.6 0 0 0-1.45-1H3a2 2 0 1 1 0-4h.08A1.6 1.6 0 0 0 4.6 9a1.6 1.6 0 0 0-.32-1.77l-.05-.06a2 2 0 1 1 2.83-2.83l.06.06A1.6 1.6 0 0 0 9 4.6a1.6 1.6 0 0 0 1-1.45V3a2 2 0 1 1 4 0v.08A1.6 1.6 0 0 0 15 4.6a1.6 1.6 0 0 0 1.77-.32l.06-.05a2 2 0 1 1 2.83 2.83l-.06.06a1.6 1.6 0 0 0-.31 1.77 1.6 1.6 0 0 0 1.45 1H21a2 2 0 1 1 0 4h-.08a1.6 1.6 0 0 0-1.45 1Z" />
        </svg>
      );
    case "logs":
      return (
        <svg {...common}>
          <path d="M4 4h16v16H4z" />
          <path d="M8 9h8M8 13h8M8 17h5" />
        </svg>
      );
    case "panelLeft":
      return (
        <svg {...common}>
          <path d="M4 4h16v16H4z" />
          <path d="M10 4v16" />
        </svg>
      );
    case "panelLeftClose":
      return (
        <svg {...common}>
          <path d="M4 4h16v16H4z" />
          <path d="M10 4v16M16 9l-3 3 3 3" />
        </svg>
      );
    case "play":
      return (
        <svg {...common}>
          <path d="m7 4 12 8-12 8z" fill="currentColor" />
        </svg>
      );
    case "stop":
      return (
        <svg {...common}>
          <rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor" />
        </svg>
      );
    case "trash":
      return (
        <svg {...common}>
          <path d="M4 7h16" />
          <path d="M10 11v6M14 11v6" />
          <path d="M5 7l1 13a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2l1-13" />
          <path d="M9 7V5a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2" />
        </svg>
      );
    case "edit":
      return (
        <svg {...common}>
          <path d="M4 20h4l11-11-4-4L4 16Z" />
          <path d="M14 6l4 4" />
        </svg>
      );
    case "eye":
      return (
        <svg {...common}>
          <path d="M3 12s3.2-6 9-6 9 6 9 6-3.2 6-9 6-9-6-9-6Z" />
          <circle cx="12" cy="12" r="2.5" />
        </svg>
      );
    case "copy":
      return (
        <svg {...common}>
          <rect x="8" y="8" width="11" height="11" rx="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1" />
        </svg>
      );
    case "folder":
      return (
        <svg {...common}>
          <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
        </svg>
      );
    case "moreHorizontal":
      return (
        <svg {...common}>
          <circle cx="6" cy="12" r="1.5" fill="currentColor" />
          <circle cx="12" cy="12" r="1.5" fill="currentColor" />
          <circle cx="18" cy="12" r="1.5" fill="currentColor" />
        </svg>
      );
    case "check":
      return (
        <svg {...common}>
          <path d="m5 12 4.5 4.5L20 7" />
        </svg>
      );
    case "minus":
      return (
        <svg {...common}>
          <path d="M5 12h14" />
        </svg>
      );
    case "square":
      return (
        <svg {...common}>
          <rect x="6" y="6" width="12" height="12" rx="1.5" />
        </svg>
      );
    case "table":
      return (
        <svg {...common}>
          <rect x="3.5" y="4.5" width="17" height="15" rx="1.5" />
          <path d="M3.5 10h17M9.5 10v9.5M3.5 15h17" />
        </svg>
      );
    case "cards":
      return (
        <svg {...common}>
          <rect x="3.5" y="4.5" width="7.5" height="7" rx="1.5" />
          <rect x="13" y="4.5" width="7.5" height="7" rx="1.5" />
          <rect x="3.5" y="13.5" width="7.5" height="7" rx="1.5" />
          <rect x="13" y="13.5" width="7.5" height="7" rx="1.5" />
        </svg>
      );
    case "x":
      return (
        <svg {...common}>
          <path d="M6 6l12 12M18 6l-6 6-6 6" />
        </svg>
      );
  }
}
