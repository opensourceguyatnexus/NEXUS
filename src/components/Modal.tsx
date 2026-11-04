// A styled in-app modal used for confirmations, replacing the browser's native
// confirm dialog so everything matches the app's design.

import { useEffect } from "react";
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
  // Close on Escape.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className={`modal-head ${danger ? "danger" : ""}`}>
          {icon && <div className="mi">{icon}</div>}
          <h3>{title}</h3>
        </div>
        <div>{children}</div>
        <div className="actions">{footer}</div>
      </div>
    </div>
  );
}
