// Account Switcher: one click between saved game accounts, plus browser profiles.
// Steam lists its own remembered accounts. For other launchers you sign in once,
// save that login under a name, then switch whenever you like. Passwords are never
// seen or stored; the backend closes the launcher before it touches any login.

import { useEffect, useState } from "react";
import { Globe, LogIn, Plus, RefreshCw, Save, Trash2, Users } from "lucide-react";
import { api, faviconUrl } from "../api";
import type { AccountEntry, AccountPlatform, AccountsOverview, BrowserInfo } from "../types";
import { useNexus } from "../state";
import { Loading, ErrorBanner, EmptyState } from "../components/ui";
import { Modal } from "../components/Modal";

const DOMAINS: Record<string, string> = {
  steam: "steampowered.com",
  riot: "riotgames.com",
  discord: "discord.com",
  epic: "epicgames.com",
  ubisoft: "ubisoft.com",
  chrome: "google.com",
  edge: "microsoft.com",
  brave: "brave.com",
  firefox: "firefox.com",
};

type Pending =
  | { kind: "switch"; platform: AccountPlatform; account: AccountEntry }
  | { kind: "save"; platform: AccountPlatform; name: string }
  | { kind: "add"; platform: AccountPlatform }
  | { kind: "delete"; platform: AccountPlatform; account: AccountEntry };

const logoStyle = { width: 22, height: 22, borderRadius: 5, flexShrink: 0 } as const;
const headStyle = { display: "flex", alignItems: "center", gap: 10, marginBottom: 12 } as const;

// The logo comes from the web, so it simply disappears when offline.
function Logo({ domain }: { domain: string }) {
  const [failed, setFailed] = useState(false);
  if (failed) return null;
  return <img src={faviconUrl(domain)} alt="" style={logoStyle} onError={() => setFailed(true)} />;
}

export function Accounts() {
  const { setNotice } = useNexus();
  const [data, setData] = useState<AccountsOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  // Errors stay in the dialog: they can hold a backup path or a next step.
  const [runError, setRunError] = useState<string | null>(null);

  const load = async () => {
    try {
      setData(await api.accountsOverview());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    load();
  }, []);

  const run = async () => {
    if (!pending) return;
    setBusy(true);
    setRunError(null);
    try {
      const { platform } = pending;
      if (pending.kind === "switch") {
        await api.accountSwitch(platform.id, pending.account.id);
        setNotice(`Switching ${platform.name} to ${pending.account.name}.`);
      } else if (pending.kind === "save") {
        await api.accountSave(platform.id, pending.name);
        setNotice(`Saved ${pending.name}.`);
      } else if (pending.kind === "add") {
        await api.accountAddNew(platform.id);
        setNotice(`${platform.name} is opening. Sign in with the other account.`);
      } else {
        await api.accountDelete(platform.id, pending.account.id);
        setNotice(`Removed ${pending.account.name}.`);
      }
      setPending(null);
      await load();
    } catch (e) {
      setRunError(String(e));
      await load();
    } finally {
      setBusy(false);
    }
  };

  const openProfile = async (browser: BrowserInfo, id: string, name: string) => {
    try {
      await api.browserOpenProfile(browser.id, id);
      setNotice(`Opening ${name} in ${browser.name}.`);
    } catch (e) {
      setNotice(String(e));
    }
  };

  const closeDialog = () => {
    if (busy) return;
    setPending(null);
    setRunError(null);
  };

  if (!data && !error) return <Loading label="Finding your accounts" />;

  const saveName = pending?.kind === "save" ? pending.name.trim() : "";

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>Account Switcher</h1>
          <p>Change game accounts in one click. Passwords are never stored.</p>
        </div>
        <button className="btn" onClick={load}>
          <RefreshCw size={15} /> Refresh
        </button>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="section-title">Games</div>
      {data && data.platforms.length === 0 ? (
        <EmptyState title="No supported launchers found" hint="Steam, Riot, Discord, Epic and Ubisoft are supported." />
      ) : (
        data?.platforms.map((p) => (
          <div className="card" key={p.id} style={{ marginBottom: 14 }}>
            <div style={headStyle}>
              <Logo domain={DOMAINS[p.id]} />
              <strong style={{ flex: 1 }}>{p.name}</strong>
              {p.running && (
                <span className="badge ok">
                  <span className="dot" /> Running
                </span>
              )}
              {!p.installed && <span className="badge off">Not installed</span>}
              {p.managed && p.installed && (
                <button
                  className="btn sm"
                  onClick={() => setPending({ kind: "save", platform: p, name: p.current ?? "" })}
                >
                  <Save size={13} /> Save current
                </button>
              )}
              {p.installed && (
                <button className="btn sm" onClick={() => setPending({ kind: "add", platform: p })}>
                  <Plus size={13} /> Add account
                </button>
              )}
            </div>

            {p.accounts.length === 0 ? (
              <p className="muted" style={{ margin: 0 }}>
                {p.managed
                  ? `Sign in to ${p.name} with "stay signed in", then press Save current.`
                  : "No remembered accounts yet. Press Add account to sign in."}
              </p>
            ) : (
              p.accounts.map((a) => (
                <div className="row" key={a.id}>
                  {a.avatar && <img src={a.avatar} alt="" style={{ ...logoStyle, borderRadius: "50%" }} />}
                  <span className="meta" style={{ flex: 1, marginLeft: a.avatar ? 10 : 0 }}>
                    <span className="name">{a.name}</span>
                    {(a.detail || !a.remembered) && (
                      <span className="path">
                        {a.detail}
                        {!a.remembered && !p.managed && " · Steam will ask for the password"}
                      </span>
                    )}
                  </span>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    {a.active && (
                      <span className={`badge ${p.verified ? "ok" : ""}`}>
                        {p.verified ? (
                          <>
                            <span className="dot" /> Current
                          </>
                        ) : (
                          "Last used"
                        )}
                      </span>
                    )}
                    {/* Discord and Ubisoft cannot prove who is signed in, so Switch stays available. */}
                    {p.installed && (!a.active || !p.verified) && (a.remembered || !p.managed) && (
                      <button
                        className="btn primary sm"
                        onClick={() => setPending({ kind: "switch", platform: p, account: a })}
                      >
                        <LogIn size={13} /> Switch
                      </button>
                    )}
                    {p.managed && (
                      <button
                        className="btn ghost icon sm"
                        title="Remove from NEXUS"
                        aria-label={`Remove ${a.name}`}
                        onClick={() => setPending({ kind: "delete", platform: p, account: a })}
                      >
                        <Trash2 size={14} />
                      </button>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        ))
      )}

      {data && data.browsers.length > 0 && (
        <>
          <div className="section-title">Browsers</div>
          {data.browsers.map((b) => (
            <div className="card" key={b.id} style={{ marginBottom: 14 }}>
              <div style={headStyle}>
                <Logo domain={DOMAINS[b.id]} />
                <strong style={{ flex: 1 }}>{b.name}</strong>
              </div>
              {b.profiles.map((pr) => (
                <div className="row" key={pr.id}>
                  <span className="meta">
                    <span className="name">{pr.name}</span>
                    {pr.detail && <span className="path">{pr.detail}</span>}
                  </span>
                  <button className="btn sm" onClick={() => openProfile(b, pr.id, pr.name)}>
                    <Globe size={13} /> Open
                  </button>
                </div>
              ))}
            </div>
          ))}
        </>
      )}

      {pending && (
        <Modal
          title={
            pending.kind === "switch"
              ? `Switch to ${pending.account.name}?`
              : pending.kind === "save"
                ? `Save this ${pending.platform.name} account`
                : pending.kind === "add"
                  ? `Add a ${pending.platform.name} account`
                  : `Remove ${pending.account.name}?`
          }
          icon={pending.kind === "delete" ? <Trash2 size={20} /> : <Users size={20} />}
          danger={pending.kind === "delete"}
          onClose={closeDialog}
          footer={
            <>
              <button className="btn ghost" onClick={closeDialog} disabled={busy}>
                Cancel
              </button>
              <button
                className={`btn ${pending.kind === "delete" ? "danger" : "primary"}`}
                onClick={run}
                disabled={busy || (pending.kind === "save" && !saveName)}
              >
                {busy
                  ? "Working..."
                  : pending.kind === "switch"
                    ? "Switch"
                    : pending.kind === "save"
                      ? "Save"
                      : pending.kind === "add"
                        ? "Continue"
                        : "Remove"}
              </button>
            </>
          }
        >
          {pending.kind === "switch" && (
            <p>
              {pending.platform.name} will close and reopen on this account.
              {" "}Close any game first.
              {pending.platform.managed &&
                " If you are signed in to an account that is not saved here, save it first or you will have to sign in to it again."}
            </p>
          )}
          {pending.kind === "save" && (
            <div className="field">
              <label>Account name</label>
              <input
                type="text"
                maxLength={60}
                autoFocus
                value={pending.name}
                onChange={(e) => setPending({ ...pending, name: e.target.value })}
              />
              <p>{pending.platform.name} will close while the login is saved. Sign in first, and tick "stay signed in".</p>
            </div>
          )}
          {pending.kind === "add" && (
            <p>
              {pending.platform.id === "steam"
                ? "Steam will close and reopen on its account picker. Close any game first."
                : `${pending.platform.name} will close and reopen signed out. Close any game first. Save the current account first if you want to come back to it. After signing in, press Save current.`}
            </p>
          )}
          {pending.kind === "delete" && (
            <p>This only removes the saved login from NEXUS. Your {pending.platform.name} account is not affected.</p>
          )}
          {runError && <ErrorBanner message={runError} />}
        </Modal>
      )}
    </div>
  );
}
