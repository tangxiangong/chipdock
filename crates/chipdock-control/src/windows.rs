use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
    time::Duration,
};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use windows_sys::Win32::{
    Foundation::{ERROR_PIPE_BUSY, HANDLE, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    System::{
        Pipes::GetNamedPipeServerProcessId,
        Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};

struct LocalMemory(*mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // The conversion APIs allocate with LocalAlloc and transfer ownership.
        unsafe { LocalFree(self.0) };
    }
}

fn sid_for(process: HANDLE) -> io::Result<String> {
    let mut token = ptr::null_mut();
    // OpenProcessToken returns an owned token handle on success.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut bytes = 0;
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &mut bytes,
        )
    };
    if bytes < size_of::<TOKEN_USER>() as u32 {
        return Err(io::Error::last_os_error());
    }
    let mut data = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            data.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // TOKEN_USER is initialized by GetTokenInformation in the aligned buffer.
    let user = unsafe { &*data.as_ptr().cast::<TOKEN_USER>() };
    let mut wide = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut wide) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _memory = LocalMemory(wide.cast());
    // ConvertSidToStringSidW returns a NUL-terminated UTF-16 string.
    let mut len = 0;
    while unsafe { *wide.add(len) } != 0 {
        len += 1;
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(wide, len) }).map_err(io::Error::other)
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
    let mut descriptor = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _memory = LocalMemory(descriptor);
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
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
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                tokio::time::sleep(Duration::from_millis(30)).await
            }
            Err(error) => return Err(error),
        }
    };
    let mut server = 0;
    // Verify a different user did not preempt the per-user pipe name.
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut server) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, server) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    if sid_for(process.as_raw_handle())? != sid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ChipDock IPC peer is a different user",
        ));
    }
    Ok(pipe)
}
