//! Bounded subprocess capture. Drop kills descendants on cancellation and timeout.
use std::{ffi::OsStr, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
};

struct Tree {
    #[cfg(unix)]
    pid: i32,
    #[cfg(windows)]
    job: isize,
}
impl Tree {
    fn attach(child: &Child) -> Result<Self, String> {
        #[cfg(unix)]
        {
            Ok(Self {
                pid: child.id().ok_or("Missing process id")? as i32,
            })
        }
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::{
                Foundation::*,
                System::{JobObjects::*, Threading::*},
            };
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let process = child.raw_handle().ok_or("Missing process handle")?;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(job, process as HANDLE) == 0
            {
                let error = std::io::Error::last_os_error().to_string();
                CloseHandle(job);
                return Err(error);
            }
            Ok(Self { job: job as isize })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = child;
            Err("Unsupported subprocess platform".into())
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-self.pid, libc::SIGKILL);
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job as _);
        }
    }
}
async fn read(mut reader: impl AsyncRead + Unpin, max: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = reader.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(bytes);
        }
        if (bytes.len() as u64).saturating_add(n as u64) > max {
            return Err("Speedtest process output exceeded limit".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
}
pub async fn output(
    program: impl AsRef<OsStr>,
    args: &[&str],
    timeout: Duration,
    max: u64,
) -> Result<Vec<u8>, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not start Speedtest process: {e}"))?;
    let _tree = Tree::attach(&child)?;
    let stdout = child.stdout.take().ok_or("Missing Speedtest stdout")?;
    let stderr = child.stderr.take().ok_or("Missing Speedtest stderr")?;
    let work = async {
        let (out, err, status) =
            tokio::try_join!(read(stdout, max), read(stderr, 16_384), async {
                child.wait().await.map_err(|e| e.to_string())
            })?;
        if !status.success() {
            let detail = String::from_utf8_lossy(&err);
            return Err(if detail.contains("NoNetworkConnection") {
                "The Internet connection appears to be offline.".into()
            } else {
                format!("Speedtest CLI failed ({status}): {}", detail.trim())
            });
        }
        Ok(out)
    };
    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| "Speedtest process timed out".to_string())?
}
