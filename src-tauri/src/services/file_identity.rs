use std::fs::Metadata;
use std::path::Path;

use crate::models::operation::FileSnapshot;

pub fn snapshot_file(path: &Path) -> Result<FileSnapshot, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() {
        return Err("源路径不是普通文件。".into());
    }
    Ok(snapshot(path, &metadata, "file"))
}

pub fn snapshot_directory(path: &Path) -> Result<FileSnapshot, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_dir() {
        return Err("目标路径不是普通目录。".into());
    }
    Ok(snapshot(path, &metadata, "directory"))
}

pub fn snapshot_matches(path: &Path, expected: &FileSnapshot) -> Result<(), String> {
    let actual = snapshot_file(path)?;
    if actual != *expected {
        return Err("文件在预览后发生变化，请重新预览。".into());
    }
    Ok(())
}

fn snapshot(path: &Path, metadata: &Metadata, kind: &str) -> FileSnapshot {
    FileSnapshot {
        kind: kind.into(),
        size: metadata.len(),
        modified_ms: metadata.modified().ok().and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_millis() as i64)
        }),
        file_identity: file_identity(path, metadata),
        volume_id: volume_id(path, metadata),
    }
}

#[cfg(unix)]
fn file_identity(_path: &Path, metadata: &Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    Some(format!("unix:{}:{}", metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn file_identity(path: &Path, _metadata: &Metadata) -> Option<String> {
    windows_file_info(path).map(|(volume, index)| format!("windows:{volume}:{index}"))
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_path: &Path, _metadata: &Metadata) -> Option<String> {
    None
}

#[cfg(unix)]
fn volume_id(_path: &Path, metadata: &Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    Some(format!("unix:{}", metadata.dev()))
}

#[cfg(windows)]
fn volume_id(path: &Path, _metadata: &Metadata) -> Option<String> {
    windows_file_info(path).map(|(volume, _)| format!("windows:{volume}"))
}

#[cfg(not(any(unix, windows)))]
fn volume_id(_path: &Path, _metadata: &Metadata) -> Option<String> {
    None
}

#[cfg(windows)]
fn windows_file_info(path: &Path) -> Option<(u32, u64)> {
    use std::ffi::c_void;
    use std::fs::OpenOptions;
    use std::mem::MaybeUninit;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

    #[repr(C)]
    struct FileTime {
        low_date_time: u32,
        high_date_time: u32,
    }

    #[repr(C)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    let file = OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .ok()?;
    let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) != 0 };
    if !succeeded {
        return None;
    }
    let information = unsafe { information.assume_init() };
    Some((
        information.volume_serial_number,
        (u64::from(information.file_index_high) << 32) | u64::from(information.file_index_low),
    ))
}
