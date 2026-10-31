import {
  useMemo,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type UIEvent,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useT } from "@veil/i18n";
import type { AppErrorCode, Profile } from "@veil/shared";
import { Checkbox, IconButton } from "@veil/ui";
import { Icon } from "../../shell/Icon";
import { useAppStore } from "../../app/store";
import { formatLastLaunched, osLabel } from "./format";
import type { LaunchDiagnostics } from "./launchDiagnostics";
import { StatusBadge } from "./StatusBadge";
import type { SortDir, SortKey } from "./filter";

type ColumnKey = SortKey | "select" | "proxy" | "os" | "notes" | "actions";
type ColumnWidths = Record<ColumnKey, number>;

interface ColumnDef {
  key: ColumnKey;
  labelKey: string;
  sortable: boolean;
  width: number;
  minWidth: number;
  maxWidth: number;
  resizable: boolean;
}

const COLUMNS: ColumnDef[] = [
  {
    key: "select",
    labelKey: "",
    sortable: false,
    width: 44,
    minWidth: 44,
    maxWidth: 44,
    resizable: false,
  },
  {
    key: "name",
    labelKey: "profiles.column.name",
    sortable: true,
    width: 220,
    minWidth: 140,
    maxWidth: 720,
    resizable: true,
  },
  {
    key: "status",
    labelKey: "profiles.column.status",
    sortable: true,
    width: 120,
    minWidth: 92,
    maxWidth: 168,
    resizable: true,
  },
  {
    key: "browserVersion",
    labelKey: "profiles.column.browserVersion",
    sortable: true,
    width: 230,
    minWidth: 136,
    maxWidth: 720,
    resizable: true,
  },
  {
    key: "proxy",
    labelKey: "profiles.column.proxy",
    sortable: false,
    width: 200,
    minWidth: 124,
    maxWidth: 640,
    resizable: true,
  },
  {
    key: "os",
    labelKey: "profiles.column.os",
    sortable: false,
    width: 112,
    minWidth: 84,
    maxWidth: 160,
    resizable: true,
  },
  {
    key: "platform",
    labelKey: "profiles.column.platform",
    sortable: true,
    width: 160,
    minWidth: 104,
    maxWidth: 360,
    resizable: true,
  },
  {
    key: "lastLaunchedAt",
    labelKey: "profiles.column.lastLaunched",
    sortable: true,
    width: 176,
    minWidth: 120,
    maxWidth: 300,
    resizable: true,
  },
  {
    key: "notes",
    labelKey: "profiles.column.notes",
    sortable: false,
    width: 180,
    minWidth: 96,
    maxWidth: 720,
    resizable: true,
  },
  {
    key: "actions",
    labelKey: "profiles.column.actions",
    sortable: false,
    width: 116,
    minWidth: 116,
    maxWidth: 116,
    resizable: false,
  },
];

const ROW_HEIGHT = 56;
const RESIZE_STEP = 16;
const DEFAULT_COLUMN_WIDTHS = COLUMNS.reduce((widths, column) => {
  widths[column.key] = column.width;
  return widths;
}, {} as ColumnWidths);
const MIN_COLUMN_WIDTH_TOTAL = COLUMNS.reduce((sum, column) => sum + column.minWidth, 0);
const MAX_COLUMN_WIDTH_TOTAL = COLUMNS.reduce((sum, column) => sum + column.maxWidth, 0);

function createAdaptiveColumnWidths(availableWidth: number): ColumnWidths {
  if (availableWidth <= 0) return DEFAULT_COLUMN_WIDTHS;
  const targetWidth = clampTableWidth(availableWidth);
  const sourceWidths = COLUMNS.map((column) => column.width);
  return distributeColumnWidths(sourceWidths, targetWidth);
}

function fitColumnWidthsToAvailableWidth(
  widths: ColumnWidths,
  availableWidth: number,
): ColumnWidths {
  if (availableWidth <= 0) return widths;
  const targetWidth = clampTableWidth(availableWidth);
  const sourceWidths = COLUMNS.map((column) => widths[column.key]);
  return distributeColumnWidths(sourceWidths, targetWidth);
}

function distributeColumnWidths(sourceWidths: number[], targetWidth: number): ColumnWidths {
  const currentTotal = sourceWidths.reduce((sum, width) => sum + width, 0);
  const delta = targetWidth - currentTotal;
  if (delta === 0) return widthsToRecord(sourceWidths);
  const capacities = COLUMNS.map((column, index) =>
    column.resizable
      ? delta > 0
        ? column.maxWidth - sourceWidths[index]!
        : sourceWidths[index]! - column.minWidth
      : 0,
  );
  const capacityTotal = capacities.reduce((sum, capacity) => sum + Math.max(0, capacity), 0);
  if (capacityTotal <= 0) return widthsToRecord(sourceWidths);

  const widths = sourceWidths.map((width, index) => {
    const column = COLUMNS[index]!;
    const capacity = Math.max(0, capacities[index] ?? 0);
    const adjusted = width + (delta * capacity) / capacityTotal;
    return Math.min(column.maxWidth, Math.max(column.minWidth, adjusted));
  });

  return normalizeColumnWidths(widths, targetWidth);
}

function clampTableWidth(availableWidth: number): number {
  return Math.min(
    MAX_COLUMN_WIDTH_TOTAL,
    Math.max(MIN_COLUMN_WIDTH_TOTAL, Math.floor(availableWidth)),
  );
}

function widthsToRecord(widths: number[]): ColumnWidths {
  return COLUMNS.reduce((result, column, columnIndex) => {
    result[column.key] = Math.round(widths[columnIndex]!);
    return result;
  }, {} as ColumnWidths);
}

function normalizeColumnWidths(widths: number[], targetWidth: number): ColumnWidths {
  const rounded = widths.map((width) => Math.round(width));
  let total = rounded.reduce((sum, width) => sum + width, 0);
  let delta = targetWidth - total;
  let index = 0;

  while (delta !== 0 && index < COLUMNS.length * 4) {
    const columnIndex = index % COLUMNS.length;
    const column = COLUMNS[columnIndex]!;
    const step = delta > 0 ? 1 : -1;
    const nextWidth = rounded[columnIndex]! + step;
    if (column.resizable && nextWidth >= column.minWidth && nextWidth <= column.maxWidth) {
      rounded[columnIndex] = nextWidth;
      total += step;
      delta -= step;
    }
    index += 1;
  }

  return COLUMNS.reduce((result, column, columnIndex) => {
    result[column.key] = rounded[columnIndex]!;
    return result;
  }, {} as ColumnWidths);
}

function resizeColumnWidths(
  currentWidths: ColumnWidths,
  key: ColumnKey,
  requestedWidth: number,
): ColumnWidths {
  const column = COLUMNS.find((candidate) => candidate.key === key);
  if (!column) return currentWidths;

  const currentWidth = currentWidths[key];
  const desiredWidth = clampColumnWidth(column, requestedWidth);
  const desiredDelta = desiredWidth - currentWidth;
  if (desiredDelta === 0) return currentWidths;

  const balanceColumns = getBalanceColumns(key);
  const balanceCapacity = balanceColumns.reduce((sum, balanceColumn) => {
    const width = currentWidths[balanceColumn.key];
    const capacity =
      desiredDelta > 0 ? width - balanceColumn.minWidth : balanceColumn.maxWidth - width;
    return sum + Math.max(0, capacity);
  }, 0);
  const appliedDelta =
    desiredDelta > 0
      ? Math.min(desiredDelta, balanceCapacity)
      : -Math.min(Math.abs(desiredDelta), balanceCapacity);

  if (appliedDelta === 0) return currentWidths;

  const nextWidths = { ...currentWidths, [key]: currentWidth + appliedDelta };
  let remaining = Math.abs(appliedDelta);

  for (const balanceColumn of balanceColumns) {
    if (remaining <= 0) break;
    const width = nextWidths[balanceColumn.key];
    const change =
      appliedDelta > 0
        ? Math.min(remaining, width - balanceColumn.minWidth)
        : Math.min(remaining, balanceColumn.maxWidth - width);

    if (change <= 0) continue;
    nextWidths[balanceColumn.key] = width + (appliedDelta > 0 ? -change : change);
    remaining -= change;
  }

  return nextWidths;
}

function clampColumnWidth(column: ColumnDef, width: number): number {
  return Math.min(column.maxWidth, Math.max(column.minWidth, Math.round(width)));
}

function getBalanceColumns(key: ColumnKey): ColumnDef[] {
  const columnIndex = COLUMNS.findIndex((column) => column.key === key);
  if (columnIndex < 0) return [];
  return [...COLUMNS.slice(columnIndex + 1), ...COLUMNS.slice(0, columnIndex)].filter(
    (column) => column.key !== key && column.resizable,
  );
}

export interface ProfilesTableProps {
  profiles: Profile[];
  /** id → display name for the proxy column. */
  proxyNames: Map<string, string>;
  /** id → launch/stop error code surfaced by the backend. */
  launchErrors: Record<string, AppErrorCode | null | undefined>;
  /** id → launch diagnostics computed from profile, runtime, and proxy state. */
  launchDiagnostics: Record<string, LaunchDiagnostics | undefined>;
  sortKey: SortKey;
  sortDir: SortDir;
  onChangeSort: (key: SortKey) => void;
  onLaunch: (profile: Profile) => void;
  onStop: (profile: Profile) => void;
  onEdit: (profile: Profile) => void;
  onModifyProxy: (profile: Profile) => void;
  onDelete: (profile: Profile) => void;
}

export function ProfilesTable({
  profiles,
  proxyNames,
  launchErrors,
  launchDiagnostics,
  sortKey,
  sortDir,
  onChangeSort,
  onLaunch,
  onStop,
  onEdit,
  onModifyProxy,
  onDelete,
}: ProfilesTableProps): ReactNode {
  const t = useT();
  const parentRef = useRef<HTMLDivElement>(null);
  const resizeRef = useRef<{ key: ColumnKey; startX: number; startWidth: number } | null>(null);
  const [columnWidths, setColumnWidths] = useState<ColumnWidths>(DEFAULT_COLUMN_WIDTHS);
  const [hasCustomWidths, setHasCustomWidths] = useState(false);
  const [scrollLeft, setScrollLeft] = useState(0);
  const { selectedProfileIds, toggleSelection, replaceSelection, clearSelection } = useAppStore();

  const visibleIds = useMemo(() => profiles.map((profile) => profile.id), [profiles]);
  const allVisibleSelected =
    visibleIds.length > 0 && visibleIds.every((id) => selectedProfileIds.has(id));
  const someVisibleSelected =
    !allVisibleSelected && visibleIds.some((id) => selectedProfileIds.has(id));

  const virtualizer = useVirtualizer({
    count: profiles.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
  });

  useLayoutEffect(() => {
    const body = parentRef.current;
    if (!body) return;

    function fitColumnsToBody(): void {
      if (!body) return;
      setColumnWidths((currentWidths) =>
        hasCustomWidths
          ? fitColumnWidthsToAvailableWidth(currentWidths, body.clientWidth)
          : createAdaptiveColumnWidths(body.clientWidth),
      );
    }

    fitColumnsToBody();
    const resizeObserver = new ResizeObserver(fitColumnsToBody);
    resizeObserver.observe(body);
    return () => resizeObserver.disconnect();
  }, [hasCustomWidths]);

  const gridStyle = useMemo(() => {
    const widths = COLUMNS.map((column) => `${columnWidths[column.key]}px`);
    const totalWidth = COLUMNS.reduce((sum, column) => sum + columnWidths[column.key], 0);
    return {
      ["--profiles-grid" as keyof CSSProperties]: widths.join(" "),
      ["--profiles-grid-width" as keyof CSSProperties]: `${totalWidth}px`,
    } as CSSProperties;
  }, [columnWidths]);

  function toggleSelectAll(): void {
    if (allVisibleSelected) clearSelection();
    else replaceSelection(visibleIds);
  }

  function handleBodyScroll(event: UIEvent<HTMLDivElement>): void {
    setScrollLeft(event.currentTarget.scrollLeft);
  }

  function setColumnWidth(key: ColumnKey, width: number): void {
    setHasCustomWidths(true);
    setColumnWidths((prev) => resizeColumnWidths(prev, key, width));
  }

  function startColumnResize(event: ReactPointerEvent<HTMLButtonElement>, column: ColumnDef): void {
    if (!column.resizable) return;
    event.preventDefault();
    event.stopPropagation();
    resizeRef.current = {
      key: column.key,
      startX: event.clientX,
      startWidth: columnWidths[column.key],
    };
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    function handlePointerMove(moveEvent: PointerEvent): void {
      const state = resizeRef.current;
      if (!state) return;
      setColumnWidth(state.key, state.startWidth + moveEvent.clientX - state.startX);
    }

    function finishColumnResize(): void {
      resizeRef.current = null;
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", finishColumnResize);
      window.removeEventListener("pointercancel", finishColumnResize);
      window.removeEventListener("blur", finishColumnResize);
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", finishColumnResize, { once: true });
    window.addEventListener("pointercancel", finishColumnResize, { once: true });
    window.addEventListener("blur", finishColumnResize, { once: true });
  }

  function handleResizeKeyDown(event: KeyboardEvent<HTMLButtonElement>, column: ColumnDef): void {
    if (!column.resizable) return;
    const current = columnWidths[column.key];
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      setColumnWidth(column.key, current - RESIZE_STEP);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      setColumnWidth(column.key, current + RESIZE_STEP);
    } else if (event.key === "Home") {
      event.preventDefault();
      setColumnWidth(column.key, column.minWidth);
    } else if (event.key === "End") {
      event.preventDefault();
      setColumnWidth(column.key, column.maxWidth);
    }
  }

  function renderHeaderCell(column: ColumnDef): ReactNode {
    const active = column.sortable && sortKey === column.key;
    const indicator = active ? (sortDir === "asc" ? "▲" : "▼") : "";
    const label = column.labelKey ? t(column.labelKey) : "";
    return (
      <div
        key={column.key}
        className="profiles-table__head-cell"
        role="columnheader"
        data-column={column.key}
        data-resizable={column.resizable}
      >
        {column.key === "select" ? (
          <Checkbox
            aria-label={allVisibleSelected ? t("profiles.clearSelection") : t("common.all")}
            checked={allVisibleSelected}
            indeterminate={someVisibleSelected}
            onChange={toggleSelectAll}
          />
        ) : column.sortable ? (
          <button
            type="button"
            className="profiles-table__sort"
            data-active={active}
            onClick={() => onChangeSort(column.key as SortKey)}
          >
            <span>{label}</span>
            <span className="profiles-table__sort-indicator">{indicator}</span>
          </button>
        ) : (
          <span className="profiles-table__head-label">{label}</span>
        )}
        {column.resizable ? (
          <button
            type="button"
            className="profiles-table__resize"
            aria-label={t("profiles.resizeColumn", { column: label })}
            aria-orientation="vertical"
            aria-valuemin={column.minWidth}
            aria-valuemax={column.maxWidth}
            aria-valuenow={columnWidths[column.key]}
            role="separator"
            onPointerDown={(event) => startColumnResize(event, column)}
            onKeyDown={(event) => handleResizeKeyDown(event, column)}
          />
        ) : null}
      </div>
    );
  }

  return (
    <div className="profiles-table" style={gridStyle}>
      <div
        className="profiles-table__head"
        role="row"
        style={{ transform: `translateX(${-scrollLeft}px)` }}
      >
        {COLUMNS.map(renderHeaderCell)}
      </div>

      <div
        ref={parentRef}
        className="profiles-table__body"
        role="rowgroup"
        aria-rowcount={profiles.length}
        onScroll={handleBodyScroll}
      >
        <div
          className="profiles-table__canvas"
          style={{ height: virtualizer.getTotalSize(), position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((row) => {
            const profile = profiles[row.index];
            if (!profile) return null;
            const selected = selectedProfileIds.has(profile.id);
            const launchError = launchErrors[profile.id] ?? null;
            const diagnostics = launchDiagnostics[profile.id];
            const browserVersion = profile.browserVersion || diagnostics?.kernelVersion || "—";
            const kernelHint = diagnostics?.kernelHint ?? null;
            const proxyLabel = profile.proxyId
              ? (proxyNames.get(profile.proxyId) ?? profile.proxyId)
              : t("profiles.noProxy");
            return (
              <div
                key={profile.id}
                className="profiles-table__row"
                role="row"
                aria-selected={selected}
                data-selected={selected}
                data-runtime-error={Boolean(launchError)}
                style={{ transform: `translateY(${row.start}px)`, height: `${row.size}px` }}
              >
                <div className="profiles-table__select-cell" role="gridcell">
                  <Checkbox
                    aria-label={profile.name}
                    checked={selected}
                    onChange={() => toggleSelection(profile.id)}
                  />
                </div>
                <div role="gridcell">
                  <span className="profile-name__label" title={profile.name}>
                    {profile.name}
                  </span>
                </div>
                <div role="gridcell">
                  <StatusBadge status={profile.status} />
                </div>
                <div role="gridcell">
                  <span
                    className="profile-kernel"
                    title={
                      kernelHint === "fallback" && diagnostics
                        ? kernelHintText(t, diagnostics)
                        : browserVersion
                    }
                  >
                    {kernelHint && kernelHint !== "fallback" && diagnostics ? (
                      <span
                        className="profile-kernel__hint"
                        data-tone={kernelHint === "missing" ? "warning" : "neutral"}
                      >
                        {kernelHintText(t, diagnostics)}
                      </span>
                    ) : null}
                    <span className="profile-kernel__version">
                      {kernelHint === "fallback" && diagnostics
                        ? kernelHintText(t, diagnostics)
                        : browserVersion}
                    </span>
                  </span>
                </div>
                <div role="gridcell">
                  <button
                    type="button"
                    className="profile-proxy-button"
                    aria-label={`${t("profiles.batch.modifyProxy")}: ${profile.name}`}
                    title={t("profiles.batch.modifyProxy")}
                    onClick={() => onModifyProxy(profile)}
                  >
                    <span className="profile-proxy-button__label">{proxyLabel}</span>
                    {diagnostics?.proxyHint ? (
                      <span className="profile-cell__hint" data-tone="warning">
                        {t(
                          diagnostics.proxyHint === "authIgnored"
                            ? "profiles.diagnostics.proxyAuthIgnored"
                            : "profiles.diagnostics.proxyMissing",
                        )}
                      </span>
                    ) : null}
                  </button>
                </div>
                <div role="gridcell">{t(osLabel(profile.os))}</div>
                <div role="gridcell">{profile.platform || "—"}</div>
                <div role="gridcell">
                  {formatLastLaunched(profile.lastLaunchedAt, t("profiles.neverLaunched"))}
                </div>
                <div role="gridcell">{profile.notes || "—"}</div>
                <div className="profiles-table__actions-cell" role="gridcell">
                  <span className="profiles-table__actions">
                    {profile.status === "running" || profile.status === "launching" ? (
                      <IconButton
                        size="sm"
                        label={t("common.stop")}
                        onClick={() => onStop(profile)}
                      >
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
                    <IconButton
                      size="sm"
                      label={t("common.delete")}
                      onClick={() => onDelete(profile)}
                    >
                      <Icon name="trash" size={14} />
                    </IconButton>
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function kernelHintText(
  t: (key: string, params?: Record<string, string | number>) => string,
  diagnostics: LaunchDiagnostics,
): string {
  switch (diagnostics.kernelHint) {
    case "missing":
      return t("profiles.diagnostics.kernelMissing");
    case "fallback":
      return t("profiles.diagnostics.kernelFallback", {
        version: diagnostics.kernelVersion ?? "—",
      });
    case "current":
      return t("profiles.diagnostics.kernelCurrent");
    case null:
      return "";
  }
}
