// App shell: sidebar + the active page, plus a small transient notice toast.

import { useState } from "react";
import { AlertTriangle, CheckCircle2, Loader2, X } from "lucide-react";
import { Sidebar } from "./components/Sidebar";
import { Dashboard } from "./pages/Dashboard";
import { Applications } from "./pages/Applications";
import { Toolkit } from "./pages/Toolkit";
import { Monitor } from "./pages/Monitor";
import { TaskManager } from "./pages/TaskManager";
import { DiskManager } from "./pages/DiskManager";
import { Accounts } from "./pages/Accounts";
import { Runtimes } from "./pages/Runtimes";
import { Startup } from "./pages/Startup";
import { Setup } from "./pages/Setup";
import { Updates } from "./pages/Updates";
import { SettingsPage } from "./pages/SettingsPage";
import { isFailure, useNexus } from "./state";
import { ErrorBanner } from "./components/ui";

export type PageId =
  | "dashboard"
  | "apps"
  | "accounts"
  | "setup"
  | "updates"
  | "runtimes"
  | "startup"
  | "toolkit"
  | "monitor"
  | "tasks"
  | "disks"
  | "settings";

const IS_DEMO = import.meta.env.MODE === "demo";

// The layout needs a desktop-sized window, so phones opening the demo get it scaled to fit.
if (IS_DEMO) {
  document.querySelector('meta[name="viewport"]')?.setAttribute("content", "width=1100");
}

// The website's demo can open on any page; the desktop app always starts on the dashboard.
const firstPage = (IS_DEMO && (new URLSearchParams(location.search).get("page") as PageId)) || "dashboard";

export default function App() {
  const [page, setPage] = useState<PageId>(firstPage);
  const { notice, setNotice, error, job, lastJobResult, clearJobResult } = useNexus();
  const resultFailed = lastJobResult !== null && isFailure(lastJobResult);

  return (
    <div className="app">
      <Sidebar active={page} onNavigate={setPage} />
      <main className="main">
        {IS_DEMO && !new URLSearchParams(location.search).has("shot") && (
          <div className="demo-ribbon">This is a demo with sample data. Nothing here touches your PC.</div>
        )}
        {error && <ErrorBanner message={`NEXUS could not finish loading: ${error}`} />}
        {job && (
          <div className="note-banner accent">
            <Loader2 size={15} className="spin" />
            <span>{job.label}</span>
          </div>
        )}
        {!job && lastJobResult && (
          <div className={resultFailed ? "err-banner" : "note-banner"} role={resultFailed ? "alert" : "status"}>
            {resultFailed ? <AlertTriangle size={16} /> : <CheckCircle2 size={16} />}
            <span style={{ flex: 1 }}>{lastJobResult}</span>
            <button className="btn ghost icon sm" title="Dismiss" aria-label="Dismiss" onClick={clearJobResult}>
              <X size={15} />
            </button>
          </div>
        )}
        {page === "dashboard" && <Dashboard navigate={setPage} />}
        {page === "apps" && <Applications />}
        {page === "accounts" && <Accounts />}
        {page === "setup" && <Setup />}
        {page === "updates" && <Updates />}
        {page === "runtimes" && <Runtimes />}
        {page === "startup" && <Startup />}
        {page === "toolkit" && <Toolkit />}
        {page === "monitor" && <Monitor />}
        {page === "tasks" && <TaskManager />}
        {page === "disks" && <DiskManager />}
        {page === "settings" && <SettingsPage />}
      </main>

      {notice && (
        <div className="toast" role={isFailure(notice) ? "alert" : "status"}>
          <span className="tdot" />
          <span>{notice}</span>
          <button className="btn ghost icon sm" title="Dismiss" aria-label="Dismiss" onClick={() => setNotice(null)}>
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}
