// Disk Manager: each disk is drawn as a bar of its partitions and unallocated
// space. Select a block to act on it: change the letter or label, extend or
// shrink, format, delete, or create a partition in free space. Every change
// needs admin. Format and delete need a typed confirmation, and the backend
// refuses the EFI, MSR, Recovery and Windows partitions regardless of the UI.

import { useEffect, useMemo, useState } from "react";
import {
  HardDrive,
  RefreshCw,
  ShieldAlert,
  Lock,
  Tag,
  Type,
  MoveHorizontal,
  Eraser,
  Trash2,
  Plus,
  Loader2,
} from "lucide-react";
import { api, fmtBytes } from "../api";
import type { DiskInfo, FreeSpace, PartitionInfo, SupportedSize } from "../types";
import { useNexus } from "../state";
import { Loading, ErrorBanner } from "../components/ui";
import { Modal } from "../components/Modal";

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const FILE_SYSTEMS = ["NTFS", "exFAT", "FAT32"];

type Selection =
  | { disk: number; kind: "part"; part: PartitionInfo }
  | { disk: number; kind: "free"; free: FreeSpace };

type Op = "letter" | "label" | "resize" | "format" | "delete" | "create";

type Segment =
  | { kind: "part"; offset: number; size: number; part: PartitionInfo }
  | { kind: "free"; offset: number; size: number; free: FreeSpace };

function segmentsOf(d: DiskInfo): Segment[] {
  const segs: Segment[] = [
    ...d.partitions.map((p) => ({ kind: "part" as const, offset: p.offset, size: p.size, part: p })),
    ...d.free.map((f) => ({ kind: "free" as const, offset: f.offset, size: f.size, free: f })),
  ];
  return segs.sort((a, b) => a.offset - b.offset);
}

function partTitle(p: PartitionInfo): string {
  if (p.letter) return `${p.letter}:`;
  if (p.kind === "System") return "EFI";
  if (p.kind === "Reserved") return "MSR";
  return p.kind;
}

function partName(p: PartitionInfo): string {
  const base = p.letter ? `${p.letter}:` : `Partition ${p.number}`;
  return p.label ? `${base} ${p.label}` : base;
}

const gb = (bytes: number) => (bytes / GB).toFixed(2);
// For limits: rounding up would offer an amount that is then refused.
const gbDown = (bytes: number) => (Math.floor((bytes / GB) * 100) / 100).toFixed(2);

export function DiskManager() {
  const { setNotice, elevated, restartAsAdmin } = useNexus();
  const [disks, setDisks] = useState<DiskInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sel, setSel] = useState<Selection | null>(null);
  const [op, setOp] = useState<Op | null>(null);

  const load = async () => {
    setError(null);
    try {
      const d = await api.listDisks();
      setDisks(d);
      // Keep the selection pointing at fresh data, or drop it if it is gone.
      setSel((s) => {
        if (!s) return null;
        const disk = d.find((x) => x.number === s.disk);
        if (!disk) return null;
        if (s.kind === "part") {
          const p = disk.partitions.find((x) => x.number === s.part.number && x.offset === s.part.offset);
          return p ? { disk: s.disk, kind: "part", part: p } : null;
        }
        const f = disk.free.find((x) => x.offset === s.free.offset);
        return f ? { disk: s.disk, kind: "free", free: f } : null;
      });
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    load();
  }, []);

  const usedLetters = useMemo(
    () => new Set((disks ?? []).flatMap((d) => d.partitions.map((p) => p.letter).filter(Boolean) as string[])),
    [disks],
  );

  const done = async (msg: string) => {
    setOp(null);
    setNotice(msg);
    await load();
  };

  if (!disks && !error) return <Loading label="Reading disks" />;

  const selPart = sel?.kind === "part" ? sel.part : null;

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>Disk Manager</h1>
          <p>See every disk and partition, and resize, create, format, or delete them.</p>
        </div>
        <button className="btn" onClick={load}>
          <RefreshCw size={15} /> Refresh
        </button>
      </div>

      {error && <ErrorBanner message={error} />}

      {!elevated && (
        <div className="note-banner">
          <ShieldAlert size={16} />
          <span style={{ flex: 1 }}>
            You can view disks now. Changing partitions needs administrator rights.
          </span>
          <button className="btn sm" onClick={restartAsAdmin} style={{ flexShrink: 0 }}>
            Restart as admin
          </button>
        </div>
      )}

      <div className="note-banner accent">
        <ShieldAlert size={15} />
        <span>
          Partition changes can lose data if something goes wrong. Back up anything important
          first. EFI, MSR, Recovery, and the Windows partition are protected.
        </span>
      </div>

      {disks?.map((d) => {
        const segs = segmentsOf(d);
        const selectedHere = sel?.disk === d.number;
        return (
          <div className="card disk-card" key={d.number}>
            <div className="spread" style={{ marginBottom: 12, gap: 12 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
                <HardDrive size={18} style={{ color: "var(--fg-dim)", flexShrink: 0 }} />
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 650 }}>
                    Disk {d.number} · {d.name}
                  </div>
                  <div className="muted" style={{ fontSize: "var(--text-meta)" }}>
                    {fmtBytes(d.size)} · {d.style} · {d.bus}
                    {d.is_boot ? " · Windows disk" : ""}
                  </div>
                </div>
              </div>
              <span className={`badge ${d.health === "Healthy" ? "ok" : "off"}`}>
                <span className="dot" /> {d.health || "Unknown"}
              </span>
            </div>

            <div className="disk-bar">
              {segs.map((s) => {
                const active =
                  selectedHere &&
                  ((s.kind === "part" && sel?.kind === "part" && sel.part.number === s.part.number) ||
                    (s.kind === "free" && sel?.kind === "free" && sel.free.offset === s.free.offset));
                const usedPct =
                  s.kind === "part" && s.part.free != null && s.size > 0
                    ? ((s.size - s.part.free) / s.size) * 100
                    : 0;
                return (
                  <button
                    key={`${s.kind}-${s.offset}`}
                    className={`disk-seg ${s.kind} ${s.kind === "part" && s.part.locked ? "locked" : ""} ${active ? "active" : ""}`}
                    style={{ flexGrow: Math.max(s.size / d.size, 0.001) }}
                    title={
                      s.kind === "part"
                        ? `${partName(s.part)} · ${s.part.file_system ?? s.part.kind} · ${fmtBytes(s.size)}`
                        : `Unallocated · ${fmtBytes(s.size)}`
                    }
                    onClick={() =>
                      setSel(
                        s.kind === "part"
                          ? { disk: d.number, kind: "part", part: s.part }
                          : { disk: d.number, kind: "free", free: s.free },
                      )
                    }
                  >
                    {s.kind === "part" && <span className="fill" style={{ width: `${usedPct}%` }} />}
                    <span className="seg-text">
                      <strong>
                        {s.kind === "part" ? partTitle(s.part) : "Free"}
                        {s.kind === "part" && s.part.locked && <Lock size={10} />}
                      </strong>
                      <span>{fmtBytes(s.size)}</span>
                    </span>
                  </button>
                );
              })}
            </div>

            {selectedHere && sel && (
              <div className="disk-actions">
                {sel.kind === "part" ? (
                  <>
                    <div className="meta" style={{ minWidth: 0 }}>
                      <div style={{ fontWeight: 600 }}>{partName(sel.part)}</div>
                      <div className="muted" style={{ fontSize: "var(--text-meta)" }}>
                        {sel.part.file_system ?? "No file system"} · {sel.part.kind} · {fmtBytes(sel.part.size)}
                        {sel.part.free != null && ` · ${fmtBytes(sel.part.free)} free`}
                        {sel.part.is_boot && " · Windows"}
                      </div>
                    </div>
                    {sel.part.locked ? (
                      <span className="badge off">
                        <Lock size={12} /> Protected system partition
                      </span>
                    ) : (
                      <div className="btn-row" style={{ flexWrap: "wrap" }}>
                        {!sel.part.is_boot && (
                          <button className="btn sm" onClick={() => setOp("letter")}>
                            <Type size={14} /> Letter
                          </button>
                        )}
                        {sel.part.file_system && (
                          <button className="btn sm" onClick={() => setOp("label")}>
                            <Tag size={14} /> Label
                          </button>
                        )}
                        {sel.part.resizable && (
                          <button className="btn sm" onClick={() => setOp("resize")}>
                            <MoveHorizontal size={14} /> Extend / Shrink
                          </button>
                        )}
                        {!sel.part.is_boot && (
                          <>
                            <button className="btn sm" onClick={() => setOp("format")}>
                              <Eraser size={14} /> Format
                            </button>
                            <button className="btn danger sm" onClick={() => setOp("delete")}>
                              <Trash2 size={14} /> Delete
                            </button>
                          </>
                        )}
                      </div>
                    )}
                  </>
                ) : (
                  <>
                    <div className="meta">
                      <div style={{ fontWeight: 600 }}>Unallocated space</div>
                      <div className="muted" style={{ fontSize: "var(--text-meta)" }}>
                        {fmtBytes(sel.free.size)} free on Disk {sel.disk}
                      </div>
                    </div>
                    <button className="btn primary sm" onClick={() => setOp("create")}>
                      <Plus size={14} /> Create partition
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
        );
      })}

      {op === "letter" && selPart && (
        <LetterDialog part={selPart} used={usedLetters} onClose={() => setOp(null)} onDone={done} />
      )}
      {op === "label" && selPart && <LabelDialog part={selPart} onClose={() => setOp(null)} onDone={done} />}
      {op === "resize" && selPart && <ResizeDialog part={selPart} onClose={() => setOp(null)} onDone={done} />}
      {op === "format" && selPart && <FormatDialog part={selPart} onClose={() => setOp(null)} onDone={done} />}
      {op === "delete" && selPart && <DeleteDialog part={selPart} onClose={() => setOp(null)} onDone={done} />}
      {op === "create" && sel?.kind === "free" && (
        <CreateDialog disk={sel.disk} free={sel.free} onClose={() => setOp(null)} onDone={done} />
      )}
    </div>
  );
}

// ---- Dialogs ----

interface DialogProps {
  onClose: () => void;
  onDone: (msg: string) => Promise<void>;
}

// Shared run/busy/error handling for every dialog.
function useRunner(onDone: DialogProps["onDone"]) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const run = async (fn: () => Promise<void>, msg: string) => {
    setBusy(true);
    setErr(null);
    try {
      await fn();
      await onDone(msg);
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  };
  return { busy, err, run };
}

function Footer({
  busy,
  onClose,
  onOk,
  okLabel,
  danger,
  disabled,
}: {
  busy: boolean;
  onClose: () => void;
  onOk: () => void;
  okLabel: string;
  danger?: boolean;
  disabled?: boolean;
}) {
  return (
    <>
      <button className="btn ghost" onClick={onClose} disabled={busy}>
        Cancel
      </button>
      <button className={`btn ${danger ? "danger" : "primary"}`} onClick={onOk} disabled={busy || disabled}>
        {busy && <Loader2 size={14} className="spin" />} {okLabel}
      </button>
    </>
  );
}

function LetterDialog({ part, used, onClose, onDone }: DialogProps & { part: PartitionInfo; used: Set<string> }) {
  const { busy, err, run } = useRunner(onDone);
  const free = "CDEFGHIJKLMNOPQRSTUVWXYZ".split("").filter((l) => !used.has(l));
  const [letter, setLetter] = useState<string>(free[0] ?? "");
  return (
    <Modal
      title={`Drive letter for ${partName(part)}`}
      icon={<Type size={20} />}
      onClose={() => !busy && onClose()}
      footer={
        <>
          {part.letter && (
            <button
              className="btn ghost"
              disabled={busy}
              onClick={() =>
                run(() => api.partitionSetLetter(part.disk, part.number, null), `Removed drive letter ${part.letter}:.`)
              }
            >
              Remove letter
            </button>
          )}
          <Footer
            busy={busy}
            onClose={onClose}
            okLabel="Apply"
            disabled={!letter}
            onOk={() =>
              run(() => api.partitionSetLetter(part.disk, part.number, letter), `Drive letter set to ${letter}:.`)
            }
          />
        </>
      }
    >
      <div className="field">
        <label>New letter</label>
        <div className="letter-grid">
          {free.map((l) => (
            <button
              key={l}
              type="button"
              className={`chip-btn ${letter === l ? "active" : ""}`}
              onClick={() => setLetter(l)}
            >
              {l}:
            </button>
          ))}
        </div>
      </div>
      <p>Apps and shortcuts that point to the old letter will stop finding their files.</p>
      {err && <ErrorBanner message={err} />}
    </Modal>
  );
}

function LabelDialog({ part, onClose, onDone }: DialogProps & { part: PartitionInfo }) {
  const { busy, err, run } = useRunner(onDone);
  const [label, setLabel] = useState(part.label ?? "");
  return (
    <Modal
      title={`Rename ${partName(part)}`}
      icon={<Tag size={20} />}
      onClose={() => !busy && onClose()}
      footer={
        <Footer
          busy={busy}
          onClose={onClose}
          okLabel="Rename"
          onOk={() => run(() => api.partitionSetLabel(part.disk, part.number, label), `Renamed to "${label}".`)}
        />
      }
    >
      <div className="field">
        <label>Volume label (up to 32 characters)</label>
        <input type="text" maxLength={32} value={label} onChange={(e) => setLabel(e.target.value)} />
      </div>
      {err && <ErrorBanner message={err} />}
    </Modal>
  );
}

function ResizeDialog({ part, onClose, onDone }: DialogProps & { part: PartitionInfo }) {
  const { busy, err, run } = useRunner(onDone);
  const [range, setRange] = useState<SupportedSize | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [mode, setMode] = useState<"shrink" | "extend">("shrink");
  // How much to free (shrink) or add (extend), not the final size.
  const [amountGb, setAmountGb] = useState("");

  useEffect(() => {
    api
      .partitionSupportedSize(part.disk, part.number)
      .then((r) => {
        setRange(r);
        if (r.min >= part.size - MB && r.max > part.size + MB) setMode("extend");
      })
      .catch((e) => setLoadErr(String(e)));
  }, [part.disk, part.number]);

  const canShrink = range != null ? Math.max(part.size - range.min, 0) : 0;
  const canExtend = range != null ? Math.max(range.max - part.size, 0) : 0;
  const limit = mode === "shrink" ? canShrink : canExtend;
  const typed = Math.round((parseFloat(amountGb) * GB) / MB) * MB;
  // The GB box only has two decimals, so a value that rounds past the limit means the limit.
  const amount = typed > limit && typed - limit <= 0.01 * GB ? limit : typed;
  const valid = Number.isFinite(amount) && amount >= MB && amount <= limit;
  const newSize = mode === "shrink" ? part.size - amount : part.size + amount;

  return (
    <Modal
      title={`${mode === "shrink" ? "Shrink" : "Extend"} ${partName(part)}`}
      icon={<MoveHorizontal size={20} />}
      onClose={() => !busy && onClose()}
      footer={
        <Footer
          busy={busy}
          onClose={onClose}
          okLabel={mode === "shrink" ? `Free ${valid ? fmtBytes(amount) : "space"}` : `Add ${valid ? fmtBytes(amount) : "space"}`}
          disabled={!valid}
          onOk={() =>
            run(
              () => api.partitionResize(part.disk, part.number, newSize, part.offset, part.size),
              mode === "shrink"
                ? `Freed ${fmtBytes(amount)} from ${partName(part)}. It is now unallocated space.`
                : `Added ${fmtBytes(amount)} to ${partName(part)}.`,
            )
          }
        />
      }
    >
      {loadErr ? (
        <ErrorBanner message={loadErr} />
      ) : !range ? (
        <p>
          <Loader2 size={14} className="spin" /> Checking how much this partition can shrink or grow...
        </p>
      ) : (
        <>
          <div className="letter-grid" style={{ marginBottom: 14 }}>
            <button
              type="button"
              className={`chip-btn ${mode === "shrink" ? "active" : ""}`}
              disabled={canShrink < MB}
              onClick={() => {
                setMode("shrink");
                setAmountGb("");
              }}
            >
              Shrink (free space)
            </button>
            <button
              type="button"
              className={`chip-btn ${mode === "extend" ? "active" : ""}`}
              disabled={canExtend < MB}
              title={canExtend < MB ? "There is no unallocated space right after this partition" : undefined}
              onClick={() => {
                setMode("extend");
                setAmountGb("");
              }}
            >
              Extend (use free space)
            </button>
          </div>

          <div className="field">
            <label>
              {mode === "shrink" ? "Space to free" : "Space to add"} (GB) · up to {gbDown(limit)} GB
            </label>
            <input
              type="text"
              inputMode="decimal"
              value={amountGb}
              placeholder={mode === "shrink" ? "For example, 50" : gbDown(limit)}
              onChange={(e) => setAmountGb(e.target.value)}
              autoFocus
            />
          </div>
          <input
            type="range"
            min={0}
            max={limit}
            step={MB}
            value={valid ? amount : 0}
            onChange={(e) => setAmountGb(gb(Number(e.target.value)))}
            style={{ width: "100%", accentColor: "var(--accent)", marginBottom: 14 }}
          />

          {valid ? (
            <p>
              {mode === "shrink" ? (
                <>
                  {partName(part)} goes from {fmtBytes(part.size)} to <strong>{fmtBytes(newSize)}</strong>, and{" "}
                  <strong>{fmtBytes(amount)}</strong> becomes free (unallocated) space after it. Your files stay
                  on {part.letter ? `${part.letter}:` : "the partition"}.
                </>
              ) : (
                <>
                  {partName(part)} goes from {fmtBytes(part.size)} to <strong>{fmtBytes(newSize)}</strong>. Your
                  files are kept.
                </>
              )}
            </p>
          ) : (
            amountGb.trim() !== "" && (
              <p style={{ color: "var(--danger)" }}>Enter an amount between 0.01 and {gbDown(limit)} GB.</p>
            )
          )}
        </>
      )}
      {err && <ErrorBanner message={err} />}
    </Modal>
  );
}

function FsFields({
  fs,
  setFs,
  label,
  setLabel,
  size,
}: {
  fs: string;
  setFs: (v: string) => void;
  label: string;
  setLabel: (v: string) => void;
  size: number;
}) {
  return (
    <>
      <div className="field">
        <label>File system</label>
        <div className="letter-grid">
          {FILE_SYSTEMS.map((f) => {
            const tooBig = f === "FAT32" && size > 32 * GB;
            return (
              <button
                key={f}
                type="button"
                className={`chip-btn ${fs === f ? "active" : ""}`}
                disabled={tooBig}
                title={tooBig ? "Windows can only format FAT32 up to 32 GB" : undefined}
                onClick={() => setFs(f)}
              >
                {f}
              </button>
            );
          })}
        </div>
      </div>
      <div className="field">
        <label>Label</label>
        <input type="text" maxLength={fs === "FAT32" ? 11 : fs === "exFAT" ? 15 : 32} value={label} onChange={(e) => setLabel(e.target.value)} />
      </div>
    </>
  );
}

function TypeToConfirm({ word, value, setValue }: { word: string; value: string; setValue: (v: string) => void }) {
  return (
    <div className="field">
      <label>
        Type <span className="mono">{word}</span> to confirm
      </label>
      <input type="text" value={value} onChange={(e) => setValue(e.target.value)} autoFocus />
    </div>
  );
}

function FormatDialog({ part, onClose, onDone }: DialogProps & { part: PartitionInfo }) {
  const { busy, err, run } = useRunner(onDone);
  const [fs, setFs] = useState(part.file_system && FILE_SYSTEMS.includes(part.file_system) ? part.file_system : "NTFS");
  const [label, setLabel] = useState(part.label ?? "");
  const [confirm, setConfirm] = useState("");
  return (
    <Modal
      title={`Format ${partName(part)}?`}
      icon={<Eraser size={20} />}
      onClose={() => !busy && onClose()}
      footer={
        <Footer
          busy={busy}
          danger
          onClose={onClose}
          okLabel="Format"
          disabled={confirm !== "FORMAT"}
          onOk={() =>
            run(() => api.partitionFormat(part.disk, part.number, fs, label, part.offset, part.size), `${partName(part)} formatted as ${fs}.`)
          }
        />
      }
    >
      <p>
        <strong>Everything on {partName(part)} ({fmtBytes(part.size)}) will be erased.</strong>
      </p>
      <FsFields fs={fs} setFs={setFs} label={label} setLabel={setLabel} size={part.size} />
      <TypeToConfirm word="FORMAT" value={confirm} setValue={setConfirm} />
      {err && <ErrorBanner message={err} />}
    </Modal>
  );
}

function DeleteDialog({ part, onClose, onDone }: DialogProps & { part: PartitionInfo }) {
  const { busy, err, run } = useRunner(onDone);
  const [confirm, setConfirm] = useState("");
  return (
    <Modal
      title={`Delete ${partName(part)}?`}
      icon={<Trash2 size={20} />}
      onClose={() => !busy && onClose()}
      footer={
        <Footer
          busy={busy}
          danger
          onClose={onClose}
          okLabel="Delete partition"
          disabled={confirm !== "DELETE"}
          onOk={() => run(() => api.partitionDelete(part.disk, part.number, part.offset, part.size), `${partName(part)} deleted.`)}
        />
      }
    >
      <p>
        <strong>The partition and all {fmtBytes(part.size)} of its data will be removed.</strong> The
        space becomes unallocated, so you can create a new partition or extend the one before it.
      </p>
      <TypeToConfirm word="DELETE" value={confirm} setValue={setConfirm} />
      {err && <ErrorBanner message={err} />}
    </Modal>
  );
}

function CreateDialog({ disk, free, onClose, onDone }: DialogProps & { disk: number; free: FreeSpace }) {
  const { busy, err, run } = useRunner(onDone);
  const [sizeGb, setSizeGb] = useState(gb(free.size));
  const [fs, setFs] = useState("NTFS");
  const [label, setLabel] = useState("New Volume");
  const bytes = Math.min(Math.round(parseFloat(sizeGb) * GB / MB) * MB, free.size);
  const valid = Number.isFinite(bytes) && bytes >= 16 * MB;
  return (
    <Modal
      title={`New partition on Disk ${disk}`}
      icon={<Plus size={20} />}
      onClose={() => !busy && onClose()}
      footer={
        <Footer
          busy={busy}
          onClose={onClose}
          okLabel="Create"
          disabled={!valid || (fs === "FAT32" && bytes > 32 * GB)}
          onOk={() =>
            run(() => api.partitionCreate(disk, free.offset, bytes, fs, label), `Created a ${fmtBytes(bytes)} ${fs} partition.`)
          }
        />
      }
    >
      <p>Up to {fmtBytes(free.size)} is available. A drive letter is assigned automatically.</p>
      <div className="field">
        <label>Size (GB)</label>
        <input type="text" inputMode="decimal" value={sizeGb} onChange={(e) => setSizeGb(e.target.value)} />
      </div>
      <FsFields fs={fs} setFs={setFs} label={label} setLabel={setLabel} size={bytes} />
      {err && <ErrorBanner message={err} />}
    </Modal>
  );
}
