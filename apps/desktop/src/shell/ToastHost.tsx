import { useEffect, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Toast } from "@veil/ui";
import { useToastStore, type ToastMessage } from "../app/toastStore";
import { Icon } from "./Icon";

/**
 * Fixed top-right stack for transient notifications, mounted once by `App`.
 *
 * It sits in the shell rather than in a page because the events it reports outlive the screen
 * that started them — an engine install announces itself whether or not the Browser page is
 * still mounted.
 */
export function ToastHost(): ReactNode {
  const toasts = useToastStore((state) => state.toasts);
  const dismiss = useToastStore((state) => state.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div className="veil-toast-host">
      {toasts.map((toast) => (
        <ToastEntry key={toast.id} toast={toast} onDismiss={() => dismiss(toast.id)} />
      ))}
    </div>
  );
}

function ToastEntry({
  toast,
  onDismiss,
}: {
  toast: ToastMessage;
  onDismiss: () => void;
}): ReactNode {
  const t = useT();

  // Auto-dismiss on a timer rather than on `animationend`: under
  // `prefers-reduced-motion: reduce` the exit animation is suppressed, and an animation-driven
  // removal would leave the toast on screen forever. Keyed on the toast id, not on `onDismiss`,
  // which is a fresh closure every render.
  useEffect(() => {
    if (toast.ttlMs <= 0) return;
    const id = window.setTimeout(onDismiss, toast.ttlMs);
    return () => window.clearTimeout(id);
  }, [toast.id, toast.ttlMs]);

  return (
    <Toast
      tone={toast.tone}
      title={t(toast.titleKey, toast.params)}
      icon={<Icon name={toast.tone === "success" ? "check" : "x"} size={16} />}
      onDismiss={onDismiss}
      dismissLabel={t("common.close")}
    >
      {toast.bodyKey ? t(toast.bodyKey, toast.params) : null}
    </Toast>
  );
}
