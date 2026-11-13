// A styled in-app modal used for confirmations, replacing the browser's native
// confirm dialog so everything matches the app's design.

import { useEffect, useId, useRef } from "react";
import type { ReactNode } from "react";

export function Modal({
  title,
  icon,
  danger,
  children,
  footer,
  onClose,
}: {
  title: string;
  icon?: ReactNode;
  danger?: boolean;
  children: ReactNode;
  footer: ReactNode;
  onClose: () => void;
}) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);
  const pressedOverlay = useRef(false);

  // Take focus on open and hand it back on close.
  useEffect(() => {
    const before = document.activeElement as HTMLElement | null;
    if (!box.current?.contains(document.activeElement)) box.current?.focus();
    return () => before?.focus?.();
  }, []);

  // Close on Escape, and keep Tab inside the dialog.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") return onClose();
      const el = box.current;
      if (e.key !== "Tab" || !el) return;
      const items = Array.from(
        el.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (items.length === 0) {
        e.preventDefault();
        el.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === el || !el.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !el.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Only a press that both starts and ends on the backdrop closes the dialog,
  // so dragging a text selection out of a field does not.
  return (
    <div
      className="modal-overlay"
      onMouseDown={(e) => {
        pressedOverlay.current = e.target === e.currentTarget;
      }}
      onClick={(e) => {
        if (pressedOverlay.current && e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal" ref={box} role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1}>
        <div className={`modal-head ${danger ? "danger" : ""}`}>
          {icon && <div className="mi">{icon}</div>}
          <h3 id={titleId}>{title}</h3>
        </div>
        <div>{children}</div>
        <div className="actions">{footer}</div>
      </div>
    </div>
  );
}
