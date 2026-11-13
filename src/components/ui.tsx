// Small shared presentational pieces used across pages.

import {
  Loader2,
  Inbox,
  AlertTriangle,
  Mouse,
  Headphones,
  MonitorCog,
  CircuitBoard,
  Fan,
  AppWindow,
  Gauge,
  ShieldAlert,
} from "lucide-react";
import { useState } from "react";
import type { ReactNode } from "react";
import { faviconUrl } from "../api";
import { useNexus } from "../state";

export function Loading({ label = "Loading" }: { label?: string }) {
  return (
    <div className="loading">
      <div className="emoji">
        <Loader2 className="spin" size={26} />
      </div>
      <div>{label}</div>
    </div>
  );
}

export function EmptyState({
  title,
  hint,
  icon,
}: {
  title: string;
  hint?: string;
  icon?: ReactNode;
}) {
  return (
    <div className="empty">
      <div className="emoji">{icon ?? <Inbox size={30} />}</div>
      <div className="big">{title}</div>
      {hint && <div>{hint}</div>}
    </div>
  );
}

export function ErrorBanner({ message }: { message: string }) {
  return (
    <div className="err-banner">
      <AlertTriangle size={16} />
      {message}
    </div>
  );
}

// Restarting would cut off a running install or update, so it waits for the job.
export function RestartAsAdminButton() {
  const { restartAsAdmin, job } = useNexus();
  return (
    <button
      className="btn sm"
      style={{ flexShrink: 0 }}
      onClick={restartAsAdmin}
      disabled={job !== null}
      title={job ? "Wait for the current job to finish first." : undefined}
    >
      Restart as admin
    </button>
  );
}

// Explains what a page cannot do without administrator rights. Hidden once elevated.
export function AdminNote({ children }: { children: ReactNode }) {
  const { elevated } = useNexus();
  if (elevated) return null;
  return (
    <div className="note-banner">
      <ShieldAlert size={16} />
      <span style={{ flex: 1 }}>{children}</span>
      <RestartAsAdminButton />
    </div>
  );
}

export function Meter({ pct, warn }: { pct: number; warn?: boolean }) {
  const clamped = Math.max(0, Math.min(100, pct));
  return (
    <div className={`meter ${warn ? "warn" : ""}`}>
      <span style={{ width: `${clamped}%` }} />
    </div>
  );
}

const CATEGORY_ICON: Record<string, typeof Mouse> = {
  mouse_keyboard: Mouse,
  audio: Headphones,
  gpu: MonitorCog,
  motherboard: CircuitBoard,
  rgb_cooling: Fan,
  benchmark: Gauge,
  custom: AppWindow,
};

// Shows a real logo: the extracted executable icon if we have one, else the
// app's website favicon, else a clean category icon. Never shows letters.
export function AppLogo({
  url,
  domain,
  category,
  size = 48,
}: {
  url?: string;
  domain?: string;
  category?: string;
  size?: number;
}) {
  const [imgFailed, setImgFailed] = useState(false);
  const Icon = CATEGORY_ICON[category ?? ""] ?? AppWindow;

  // Prefer the real executable icon, then a favicon for the website.
  const src = url || (domain && !imgFailed ? faviconUrl(domain) : undefined);

  return (
    <div className="logo-tile" style={{ width: size, height: size }}>
      {src ? (
        <img
          src={src}
          alt=""
          onError={() => {
            if (!url) setImgFailed(true);
          }}
        />
      ) : (
        <Icon size={size * 0.46} strokeWidth={1.8} />
      )}
    </div>
  );
}
