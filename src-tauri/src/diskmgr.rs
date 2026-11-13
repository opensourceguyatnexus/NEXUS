// Disk Manager: a visual map of disks and partitions, plus the common partition
// operations (drive letter, label, extend/shrink, create, format, delete).
//
// Everything goes through the Windows Storage PowerShell module. Values from the
// UI are validated here and only ever reach PowerShell as numbers or as tightly
// checked strings, never as free text. Each change runs a guard first that
// re-reads the partition from Windows and refuses the EFI, MSR, Recovery and
// Windows (boot) partitions, so the protection does not depend on the UI.

use crate::elevation;
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MB: u64 = 1024 * 1024;
// Gaps smaller than this are alignment slack, not usable free space.
const MIN_GAP: u64 = 16 * MB;

#[derive(Serialize)]
pub struct PartitionInfo {
    pub disk: u32,
    pub number: u32,
    pub letter: Option<String>,
    pub offset: u64,
    pub size: u64,
    pub kind: String,
    pub file_system: Option<String>,
    pub label: Option<String>,
    pub free: Option<u64>,
    pub is_boot: bool,
    pub is_system: bool,
    pub locked: bool,
    pub resizable: bool,
}

#[derive(Serialize)]
pub struct FreeSpace {
    pub offset: u64,
    pub size: u64,
}

#[derive(Serialize)]
pub struct DiskInfo {
    pub number: u32,
    pub name: String,
    pub style: String,
    pub bus: String,
    pub size: u64,
    pub health: String,
    pub is_boot: bool,
    pub is_offline: bool,
    pub partitions: Vec<PartitionInfo>,
    pub free: Vec<FreeSpace>,
}

#[derive(Serialize)]
pub struct SupportedSize {
    pub min: u64,
    pub max: u64,
}

#[derive(Deserialize)]
struct RawDisk {
    number: u32,
    name: Option<String>,
    style: Option<String>,
    bus: Option<String>,
    size: u64,
    health: Option<String>,
    is_boot: bool,
    is_offline: bool,
}

#[derive(Deserialize)]
struct RawPart {
    disk: u32,
    number: u32,
    letter: Option<String>,
    offset: u64,
    size: u64,
    kind: Option<String>,
    is_boot: bool,
    is_system: bool,
    fs: Option<String>,
    label: Option<String>,
    free: Option<u64>,
}

#[derive(Deserialize)]
struct RawData {
    disks: Vec<RawDisk>,
    parts: Vec<RawPart>,
}

// Run a PowerShell script. Errors come back as the exception's own message.
fn ps(script: &str) -> Result<String, String> {
    let wrapped = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; $ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; \
         try {{ {} }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}",
        script
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &wrapped])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Could not run PowerShell: {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr);
        let first = msg.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Unknown error");
        if first.contains("CIM resource was not available") || first.contains("Access denied") {
            return Err("This needs administrator rights. Use Restart as admin.".into());
        }
        Err(first.to_string())
    }
}

fn need_admin() -> Result<(), String> {
    if elevation::is_elevated() {
        Ok(())
    } else {
        Err("Changing partitions needs administrator rights. Use Restart as admin.".into())
    }
}

fn is_locked_kind(kind: &str) -> bool {
    matches!(kind, "System" | "Reserved" | "Recovery")
}

pub fn list_disks() -> Result<Vec<DiskInfo>, String> {
    let script = r#"
$disks = @(Get-Disk | ForEach-Object { [pscustomobject]@{
  number=[uint32]$_.Number; name=$_.FriendlyName; style=[string]$_.PartitionStyle;
  bus=[string]$_.BusType; size=[uint64]$_.Size; health=[string]$_.HealthStatus; is_boot=[bool]$_.IsBoot;
  is_offline=[bool]$_.IsOffline } })
$parts = @(Get-Partition | ForEach-Object {
  $v = $null; try { $v = $_ | Get-Volume -ErrorAction Stop } catch {}
  [pscustomobject]@{
    disk=[uint32]$_.DiskNumber; number=[uint32]$_.PartitionNumber;
    letter=$(if ($_.DriveLetter -and [char]$_.DriveLetter -ne [char]0) { [string]$_.DriveLetter } else { $null });
    offset=[uint64]$_.Offset; size=[uint64]$_.Size; kind=[string]$_.Type;
    is_boot=[bool]$_.IsBoot; is_system=[bool]$_.IsSystem;
    fs=$(if ($v -and $v.FileSystem) { [string]$v.FileSystem } else { $null });
    label=$(if ($v) { [string]$v.FileSystemLabel } else { $null });
    free=$(if ($v) { [uint64]$v.SizeRemaining } else { $null }) } })
[pscustomobject]@{ disks=$disks; parts=$parts } | ConvertTo-Json -Depth 4 -Compress
"#;
    let text = ps(script)?;
    let raw: RawData =
        serde_json::from_str(text.trim()).map_err(|e| format!("Could not read disks: {}", e))?;

    let mut disks: Vec<DiskInfo> = raw
        .disks
        .into_iter()
        .map(|d| {
            let mut partitions: Vec<PartitionInfo> = raw
                .parts
                .iter()
                .filter(|p| p.disk == d.number)
                .map(|p| {
                    let kind = p.kind.clone().unwrap_or_default();
                    let locked = is_locked_kind(&kind) || p.is_system;
                    PartitionInfo {
                        disk: p.disk,
                        number: p.number,
                        letter: p.letter.clone(),
                        offset: p.offset,
                        size: p.size,
                        file_system: p.fs.clone(),
                        label: p.label.clone().filter(|l| !l.is_empty()),
                        free: p.free,
                        is_boot: p.is_boot,
                        is_system: p.is_system,
                        // Resizing needs a mounted NTFS/ReFS volume.
                        resizable: !locked
                            && matches!(p.fs.as_deref(), Some("NTFS") | Some("ReFS")),
                        locked,
                        kind,
                    }
                })
                .collect();
            partitions.sort_by_key(|p| p.offset);

            // Unallocated gaps between partitions and at the end of the disk.
            // GPT keeps roughly the last 1 MB for its backup table.
            let mut free = Vec::new();
            let mut cursor = MB;
            let end = if d.style == Some("GPT".into()) { d.size.saturating_sub(MB) } else { d.size };
            for p in &partitions {
                if p.offset > cursor && p.offset - cursor >= MIN_GAP {
                    free.push(FreeSpace { offset: cursor, size: p.offset - cursor });
                }
                cursor = cursor.max(p.offset + p.size);
            }
            if end > cursor && end - cursor >= MIN_GAP {
                free.push(FreeSpace { offset: cursor, size: end - cursor });
            }

            DiskInfo {
                number: d.number,
                name: d.name.unwrap_or_else(|| format!("Disk {}", d.number)),
                style: d.style.unwrap_or_default(),
                bus: d.bus.unwrap_or_default(),
                size: d.size,
                health: d.health.unwrap_or_default(),
                is_boot: d.is_boot,
                is_offline: d.is_offline,
                partitions,
                free,
            }
        })
        .collect();
    disks.sort_by_key(|d| d.number);
    Ok(disks)
}

// What the user was looking at when they confirmed. Disk and partition numbers
// can change (for example when a USB drive is swapped), so a destructive action
// refuses to run unless the partition still has this offset and size.
#[derive(Clone, Copy)]
pub struct Expected {
    pub offset: u64,
    pub size: u64,
}

// PowerShell that loads $p and refuses protected partitions. `strict` also
// refuses the Windows partition (for format and delete).
fn guard(disk: u32, part: u32, strict: bool, expected: Option<Expected>) -> String {
    let same = expected
        .map(|e| {
            format!(
                "if ([uint64]$p.Offset -ne {} -or [uint64]$p.Size -ne {}) \
                 {{ throw 'The disks changed since this page loaded. Nothing was changed. Press Refresh and try again.' }} ",
                e.offset, e.size
            )
        })
        .unwrap_or_default();
    format!(
        "$p = Get-Partition -DiskNumber {disk} -PartitionNumber {part}; {same}\
         if ($p.IsSystem -or @('System','Reserved','Recovery') -contains [string]$p.Type) \
         {{ throw 'This is a protected system partition (EFI, MSR or Recovery).' }} \
         {strict}",
        disk = disk,
        part = part,
        same = same,
        strict = if strict {
            "if ($p.IsBoot -or ($p.DriveLetter -and [string]$p.DriveLetter -eq $env:SystemDrive.Substring(0,1))) \
             { throw 'This is the Windows partition and cannot be formatted or deleted.' }"
        } else {
            ""
        }
    )
}

fn valid_letter(letter: &str) -> Option<char> {
    let mut chars = letter.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => {
            let c = c.to_ascii_uppercase();
            // A and B are reserved for floppy drives.
            if c >= 'C' {
                Some(c)
            } else {
                None
            }
        }
        _ => None,
    }
}

// Volume labels: up to 32 characters, nothing that could break quoting. PowerShell
// also treats curly quotes as quotes, so those are refused too.
fn valid_label(label: &str) -> bool {
    label.chars().count() <= 32
        && label.chars().all(|c| {
            !c.is_control()
                && !matches!(
                    c,
                    '\'' | '"' | '`' | '$' | '*' | '?' | '/' | '\\' | '|' | '<' | '>' | ':'
                        | '\u{2018}'..='\u{201F}'
                )
        })
}

// File systems have their own label limits, and Format-Volume fails past them.
fn check_label(fs: &str, label: &str) -> Result<(), String> {
    if !valid_label(label) {
        return Err("Labels can be up to 32 characters and cannot contain quotes or ` $ * ? / \\ | < > :".into());
    }
    let max = match fs {
        "FAT32" => 11,
        "exFAT" => 15,
        _ => 32,
    };
    if label.chars().count() > max {
        return Err(format!("{} labels can be up to {} characters.", fs, max));
    }
    Ok(())
}

fn valid_fs(fs: &str) -> bool {
    matches!(fs, "NTFS" | "exFAT" | "FAT32")
}

pub fn set_letter(disk: u32, part: u32, letter: Option<String>) -> Result<(), String> {
    need_admin()?;
    let g = guard(disk, part, false, None);
    let action = match letter {
        Some(l) => {
            let c = valid_letter(&l).ok_or("Choose a drive letter from C to Z.")?;
            format!(
                "if ($p.IsBoot) {{ throw 'The Windows drive letter cannot be changed.' }} \
                 if (Test-Path '{c}:\\') {{ throw 'Drive letter {c}: is already in use by another drive.' }} \
                 Set-Partition -DiskNumber {disk} -PartitionNumber {part} -NewDriveLetter {c}",
                c = c,
                disk = disk,
                part = part
            )
        }
        None => format!(
            "if ($p.IsBoot) {{ throw 'The Windows drive letter cannot be removed.' }} \
             if (-not $p.DriveLetter -or [char]$p.DriveLetter -eq [char]0) {{ throw 'This partition has no drive letter.' }} \
             Remove-PartitionAccessPath -DiskNumber {disk} -PartitionNumber {part} -AccessPath ([string]$p.DriveLetter + ':\\')",
            disk = disk,
            part = part
        ),
    };
    ps(&format!("{} {}", g, action)).map(|_| ())
}

pub fn set_label(disk: u32, part: u32, label: String) -> Result<(), String> {
    need_admin()?;
    if !valid_label(&label) {
        return Err("Labels can be up to 32 characters and cannot contain quotes or ` $ * ? / \\ | < > :".into());
    }
    ps(&format!(
        "{} $p | Get-Volume | Set-Volume -NewFileSystemLabel '{}'",
        guard(disk, part, false, None),
        label
    ))
    .map(|_| ())
}

pub fn supported_size(disk: u32, part: u32) -> Result<SupportedSize, String> {
    need_admin()?;
    let text = ps(&format!(
        "{} $s = Get-PartitionSupportedSize -DiskNumber {} -PartitionNumber {}; \
         [pscustomobject]@{{ min=[uint64]$s.SizeMin; max=[uint64]$s.SizeMax }} | ConvertTo-Json -Compress",
        guard(disk, part, false, None),
        disk,
        part
    ))?;
    #[derive(Deserialize)]
    struct Raw {
        min: u64,
        max: u64,
    }
    let r: Raw = serde_json::from_str(text.trim()).map_err(|e| format!("Could not read sizes: {}", e))?;
    Ok(SupportedSize { min: r.min, max: r.max })
}

pub fn resize(disk: u32, part: u32, size: u64, expected: Expected) -> Result<(), String> {
    need_admin()?;
    let s = supported_size(disk, part)?;
    if size < s.min || size > s.max {
        return Err("That size is outside what this partition supports.".into());
    }
    ps(&format!(
        "{} Resize-Partition -DiskNumber {} -PartitionNumber {} -Size {}",
        guard(disk, part, false, Some(expected)),
        disk,
        part,
        size
    ))
    .map(|_| ())
}

pub fn create(disk: u32, offset: u64, size: u64, fs: String, label: String) -> Result<(), String> {
    need_admin()?;
    if !valid_fs(&fs) {
        return Err("Choose NTFS, exFAT or FAT32.".into());
    }
    check_label(&fs, &label)?;
    if fs == "FAT32" && size > 32 * 1024 * MB {
        return Err("Windows can only format FAT32 up to 32 GB. Use exFAT or NTFS.".into());
    }
    // Align to 1 MB, the Windows default.
    let aligned = offset.div_ceil(MB) * MB;
    let size = size.saturating_sub(aligned - offset) / MB * MB;
    let offset = aligned;
    if size < MIN_GAP {
        return Err("The new partition must be at least 16 MB.".into());
    }
    ps(&format!(
        "$d = Get-Disk -Number {disk}; \
         if ($d.IsOffline) {{ throw 'This disk is offline. Open Windows Disk Management, right-click the disk and choose Online, then press Refresh.' }} \
         if ([string]$d.PartitionStyle -eq 'RAW') {{ throw 'This disk is not initialized. Open Windows Disk Management, right-click the disk and choose Initialize Disk, then press Refresh.' }} \
         if ($d.IsReadOnly) {{ throw 'This disk is read-only.' }} \
         $n = New-Partition -DiskNumber {disk} -Offset {offset} -Size {size} -AssignDriveLetter; \
         try {{ Format-Volume -Partition $n -FileSystem {fs} -NewFileSystemLabel '{label}' -Confirm:$false | Out-Null }} \
         catch {{ $n | Remove-Partition -Confirm:$false; throw }}",
        disk = disk,
        offset = offset,
        size = size,
        fs = fs,
        label = label
    ))
    .map(|_| ())
}

pub fn format(disk: u32, part: u32, fs: String, label: String, expected: Expected) -> Result<(), String> {
    need_admin()?;
    if !valid_fs(&fs) {
        return Err("Choose NTFS, exFAT or FAT32.".into());
    }
    check_label(&fs, &label)?;
    ps(&format!(
        "{} if ('{fs}' -eq 'FAT32' -and $p.Size -gt 32GB) {{ throw 'Windows can only format FAT32 up to 32 GB. Use exFAT or NTFS.' }} \
         Format-Volume -Partition $p -FileSystem {fs} -NewFileSystemLabel '{label}' -Confirm:$false -Force | Out-Null",
        guard(disk, part, true, Some(expected)),
        fs = fs,
        label = label
    ))
    .map(|_| ())
}

pub fn delete(disk: u32, part: u32, expected: Expected) -> Result<(), String> {
    need_admin()?;
    ps(&format!(
        "{} Remove-Partition -DiskNumber {} -PartitionNumber {} -Confirm:$false",
        guard(disk, part, true, Some(expected)),
        disk,
        part
    ))
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_refuse_quotes_and_file_system_limits() {
        assert!(valid_label("Games 2"));
        assert!(!valid_label("a\u{2019}; calc; \u{2019}"));
        assert!(!valid_label("a'b"));
        assert!(check_label("FAT32", "123456789012").is_err());
        assert!(check_label("exFAT", "123456789012").is_ok());
        assert!(check_label("NTFS", "123456789012").is_ok());
    }

    // Read-only: the guard only loads a partition and compares it, with no action
    // after it. A wrong offset must be refused, and the true one accepted.
    #[test]
    fn guard_refuses_a_partition_that_changed() {
        let disks = list_disks().unwrap();
        let p = disks
            .iter()
            .flat_map(|d| &d.partitions)
            .find(|p| p.letter.is_some() && !p.locked && !p.is_boot)
            .expect("a data partition");
        let right = Expected { offset: p.offset, size: p.size };
        let wrong = Expected { offset: p.offset + MB, size: p.size };
        assert!(ps(&guard(p.disk, p.number, true, Some(right))).is_ok());
        let err = ps(&guard(p.disk, p.number, true, Some(wrong))).unwrap_err();
        assert!(err.contains("disks changed"), "{}", err);
    }
}
