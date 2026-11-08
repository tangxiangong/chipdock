use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    time::Duration,
};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use windows::Win32::{
    Foundation::{ERROR_PIPE_BUSY, HANDLE, HLOCAL, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    },
    System::{
        Pipes::GetNamedPipeServerProcessId,
        Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};
use windows::core::{PCWSTR, PWSTR};

struct LocalMemory(HLOCAL);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // The conversion APIs allocate with LocalAlloc and transfer ownership.
        unsafe { LocalFree(Some(self.0)) };
    }
}

fn io_error(error: windows::core::Error) -> io::Error {
    let code = error.code().0 as u32;
    if code & 0xffff0000 == 0x80070000 {
        io::Error::from_raw_os_error((code & 0xffff) as i32)
    } else {
        io::Error::other(error)
    }
}

fn sid_for(process: HANDLE) -> io::Result<String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.map_err(io_error)?;
    // The token handle is owned; the process handle remains borrowed.
    let token = unsafe { OwnedHandle::from_raw_handle(token.0) };
    let handle = HANDLE(token.as_raw_handle());
    let mut bytes = 0;
    let sized = unsafe { GetTokenInformation(handle, TokenUser, None, 0, &mut bytes) };
    if bytes < size_of::<TOKEN_USER>() as u32 {
        return Err(sized.err().map(io_error).unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid token information length",
            )
        }));
    }
    let mut data = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            handle,
            TokenUser,
            Some(data.as_mut_ptr().cast()),
            bytes,
            &mut bytes,
        )
    }
    .map_err(io_error)?;
    // The API initialized TOKEN_USER inside the aligned, bounded allocation.
    let user = unsafe { &*data.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) }.map_err(io_error)?;
    let _memory = LocalMemory(HLOCAL(sid.0.cast()));
    unsafe { sid.to_string() }.map_err(io::Error::other)
}

fn current_sid() -> io::Result<String> {
    sid_for(unsafe { GetCurrentProcess() })
}

fn name(sid: &str) -> String {
    format!(r"\\.\pipe\com.xiaoyu.chipdock.{sid}")
}

fn instance(name: &str, sid: &str, first: bool) -> io::Result<NamedPipeServer> {
    let sddl: Vec<_> = format!("D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain([0])
        .collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(io_error)?;
    let _memory = LocalMemory(HLOCAL(descriptor.0));
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    // The descriptor and its attributes remain valid for the synchronous create call.
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(32)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
    }
}

pub struct Listener {
    pipe: NamedPipeServer,
    name: String,
    sid: String,
}

impl Listener {
    pub fn bind() -> io::Result<Self> {
        let sid = current_sid()?;
        let name = name(&sid);
        let pipe = instance(&name, &sid, true)?;
        Ok(Self { pipe, name, sid })
    }

    pub async fn accept(&mut self) -> io::Result<NamedPipeServer> {
        self.pipe.connect().await?;
        let next = instance(&self.name, &self.sid, false)?;
        Ok(std::mem::replace(&mut self.pipe, next))
    }
}

pub async fn connect() -> io::Result<NamedPipeClient> {
    let sid = current_sid()?;
    let name = name(&sid);
    let pipe = loop {
        match ClientOptions::new().open(&name) {
            Ok(pipe) => break pipe,
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => {
                tokio::time::sleep(Duration::from_millis(30)).await
            }
            Err(error) => return Err(error),
        }
    };
    let mut server = 0;
    // Verify a different user did not preempt the per-user pipe name.
    unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut server) }
        .map_err(io_error)?;
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, server) }
        .map_err(io_error)?;
    let process = unsafe { OwnedHandle::from_raw_handle(process.0) };
    if sid_for(HANDLE(process.as_raw_handle()))? != sid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ChipDock IPC peer is a different user",
        ));
    }
    Ok(pipe)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_api_errors_preserve_win32_error_kind() {
        let error = io_error(windows::core::Error::from_hresult(
            windows::core::HRESULT::from_win32(5),
        ));
        assert_eq!(error.raw_os_error(), Some(5));
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
