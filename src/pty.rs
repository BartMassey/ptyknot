// Most code borrowed from https://github.com/stemjail/tty-rs .
use std::path::*;
use std::io::{Result, Error, ErrorKind};
use std::os::unix::io::AsRawFd;
use std::ffi::{CStr, OsString};
use std::os::unix::ffi::OsStringExt;
use libc::{c_int, pid_t};
use libc::waitpid as raw_waitpid;

mod raw {
    use libc::{c_int, c_char};

    extern {
        pub fn grantpt(fd: c_int) -> c_int;
        pub fn unlockpt(fd: c_int) -> c_int;
        pub fn ptsname(fd: c_int) -> *const c_char;
    }
}

pub fn grantpt<T>(master: &mut T) -> Result<()> where T: AsRawFd {
    match unsafe { raw::grantpt(master.as_raw_fd()) } {
        0 => Ok(()),
        _ => Err(Error::last_os_error()),
    }
}

pub fn unlockpt<T>(master: &mut T) -> Result<()> where T: AsRawFd {
    match unsafe { raw::unlockpt(master.as_raw_fd()) } {
        0 => Ok(()),
        _ => Err(Error::last_os_error()),
    }
}

// Rust getcwd implementation used as reference.
pub fn ptsname<T>(master: &mut T) -> Result<PathBuf> where T: AsRawFd {
    let cstr = match unsafe { raw::ptsname(master.as_raw_fd()).as_ref() } {
        None => return Err(Error::last_os_error()),
        Some(ptr) => unsafe { CStr::from_ptr(ptr) }
    };
    let buf = match cstr.to_str() {
        Ok(s) => s,
        Err(e) => return Err(Error::new(ErrorKind::InvalidData, e))
    };
    let os_string = OsString::from_vec(buf.as_bytes().to_vec());
    Ok(PathBuf::from(os_string))
}

pub fn waitpid(pid: i32) -> Result<i32> {
    let mut status: c_int = 0;
    match unsafe { raw_waitpid(pid as pid_t,
                               &mut status as *mut c_int,
                               0) } {
        -1 => Err(Error::last_os_error()),
        _ => Ok(status as i32)
    }
}
