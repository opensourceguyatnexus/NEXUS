// Left navigation. Purely presentational: it reports the selected page up to App.

import {
  LayoutDashboard,
  Cpu,
  AppWindow,
  Wrench,
  Activity,
  Settings as SettingsIcon,
  PackageCheck,
  Power,
  Rocket,
  ArrowUpCircle,
  ListTree,
  HardDrive,
  Users,
} from "lucide-react";
import type { PageId } from "../App";
import { LogoMark } from "./LogoMark";
import { version } from "../../package.json";

type Item = { id: PageId; label: string; icon: typeof Cpu };

// Grouped by what you are doing: playing, setting up, then keeping the PC healthy.
const GROUPS: Item[][] = [
  [
    { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
    { id: "apps", label: "Applications", icon: AppWindow },
    { id: "accounts", label: "Account Switcher", icon: Users },
  ],
  [
    { id: "setup", label: "New PC Setup", icon: Rocket },
    { id: "updates", label: "Update Center", icon: ArrowUpCircle },
    { id: "runtimes", label: "Runtimes", icon: PackageCheck },
  ],
  [
    { id: "startup", label: "Startup Manager", icon: Power },
    { id: "toolkit", label: "Windows Toolkit", icon: Wrench },
    { id: "monitor", label: "System Monitor", icon: Activity },
    { id: "tasks", label: "Task Manager", icon: ListTree },
    { id: "disks", label: "Disk Manager", icon: HardDrive },
  ],
  [{ id: "settings", label: "Settings", icon: SettingsIcon }],
];

export function Sidebar({
  active,
  onNavigate,
}: {
  active: PageId;
  onNavigate: (p: PageId) => void;
}) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <LogoMark className="logo" />
        <div>
          <div className="name">NEXUS</div>
          <div className="sub">Control Center</div>
        </div>
      </div>

      {GROUPS.map((group, i) => (
        <div key={group[0].id}>
          {i > 0 && <div className="nav-gap" />}
          {group.map((it) => {
            const Icon = it.icon;
            return (
              <button
                key={it.id}
                className={`nav-item ${active === it.id ? "active" : ""}`}
                aria-current={active === it.id ? "page" : undefined}
                onClick={() => onNavigate(it.id)}
              >
                <Icon size={16} />
                {it.label}
              </button>
            );
          })}
        </div>
      ))}

      <div className="sidebar-footer">
        v{version}, no telemetry
      </div>
    </aside>
  );
}
