// A card for one app: real icon, name, description, status, and a prominent
// action. If installed it launches; if installed but the exe is unresolved it
// lets the user locate it; if missing it links to the official page.

import { ExternalLink, FolderSearch, Globe, Rocket, Star, Trash2 } from "lucide-react";
import type { UnifiedApp } from "../state";
import { useNexus } from "../state";
import { domainFromUrl } from "../api";
import { AppLogo } from "./ui";
import { CATEGORY_LABELS } from "../types";

export function AppCard({ app }: { app: UnifiedApp }) {
  const { settings, iconFor, launch, locateAndLaunch, toggleFavorite, openUrl, removeCustomApp } =
    useNexus();
  const fav = settings?.favorites.includes(app.id) ?? false;
  const canLaunch = app.installed && (!!app.launch_path || !!app.launch_aumid);
  const installedNoPath = app.installed && !app.launch_path && !app.launch_aumid;

  return (
    <div className="card app-card fade-in">
      <div className="top">
        <div style={{ display: "flex", gap: 13, minWidth: 0 }}>
          <AppLogo
            url={iconFor(app)}
            domain={domainFromUrl(app.download_url) ?? undefined}
            category={app.category}
          />
          <div style={{ minWidth: 0 }}>
            <h3>{app.name}</h3>
            <div className="vendor">
              {app.vendor} · {CATEGORY_LABELS[app.category] ?? app.category}
            </div>
          </div>
        </div>
        <button
          className="btn ghost icon"
          title={fav ? "Remove from favorites" : "Add to favorites"}
          onClick={() => toggleFavorite(app.id)}
          style={{ color: fav ? "var(--accent)" : undefined }}
        >
          <Star size={16} fill={fav ? "currentColor" : "none"} />
        </button>
      </div>

      <div className="desc">{app.description}</div>

      <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
        {app.installed ? (
          <span className="badge ok">
            <span className="dot" /> Installed
          </span>
        ) : (
          <span className="badge off">
            <span className="dot" /> Not installed
          </span>
        )}
        {app.custom && <span className="badge">Custom</span>}
        {installedNoPath && <span className="badge">Path not found</span>}
      </div>

      <div className="note">{app.compatibility}</div>

      <div className="btn-row">
        {canLaunch && (
          <button className="btn primary block" onClick={() => launch(app)}>
            <Rocket size={15} /> Launch App
          </button>
        )}

        {installedNoPath && (
          <button className="btn primary block" onClick={() => locateAndLaunch(app)}>
            <FolderSearch size={15} /> Locate & Launch
          </button>
        )}

        {!app.installed && (
          <>
            <button className="btn block" onClick={() => openUrl(app.download_url)}>
              <Globe size={15} /> Official Website
            </button>
            <button
              className="btn ghost icon"
              title="Already installed? Locate the .exe"
              onClick={() => locateAndLaunch(app)}
            >
              <FolderSearch size={15} />
            </button>
          </>
        )}

        {canLaunch && app.download_url && (
          <button
            className="btn ghost icon"
            title="Open official page"
            onClick={() => openUrl(app.download_url)}
          >
            <ExternalLink size={15} />
          </button>
        )}

        {app.custom && (
          <button
            className="btn danger icon"
            title="Remove custom entry"
            onClick={() => removeCustomApp(app.id)}
          >
            <Trash2 size={15} />
          </button>
        )}
      </div>
    </div>
  );
}
