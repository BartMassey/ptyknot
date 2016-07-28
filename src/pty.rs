// Copyright © 2016 Bart Massey
// This work is made available under the "MIT License".
// Please see the file COPYING in this distribution for
// license terms.

//! Low-level pseudo-tty setup routines.
//!
//! This module contains several pty setup functions and a
//! "special" `waitpid()` implementation.
//!
//! Much of this is code borrowed from <http://github.com/stemjail/tty-rs>.

use std::path::*;
use std::fs::File;
use std::io::{Result, Error, ErrorKind};
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd};
use std::ffi::{CStr, OsString};
use std::os::unix::ffi::OsStringExt;
use libc::{c_int, pid_t};

mod raw {
    use libc::{c_int, c_char};
    pub use libc::{waitpid, dup2, close, pipe};

    extern {
        pub fn grantpt(fd: c_int) -> c_int;
        pub fn unlockpt(fd: c_int) -> c_int;
        pub fn ptsname(fd: c_int) -> *const c_char;
    }
}

/// Change the mode and owner of the slave pty associated
/// with a given open master. See `grantpt(3)` in the UNIX
/// manual pages for details.
pub fn grantpt<T>(master: &mut T) -> Result<()> where T: AsRawFd {
    match unsafe { raw::grantpt(master.as_raw_fd()) } {
        -1 => Err(Error::last_os_error()),
        _ => Ok(())
    }
}

/// "Unlocks" the slave pty associated with the give nopen
/// master. See `unlockpt(3)` in the UNIX manual pages for
/// details.
pub fn unlockpt<T>(master: &mut T) -> Result<()> where T: AsRawFd {
    match unsafe { raw::unlockpt(master.as_raw_fd()) } {
        -1 => Err(Error::last_os_error()),
        _ => Ok(())
    }
}

/// Returns the name of the slave pty associated with the
/// given open master. See `unlockpt(3)` in the UNIX manual
/// pages for details.
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

/// Blocking wait until process completes. Returns the
/// process exit status. See `waitpid(3)` in the UNIX manual
/// pages for details.
pub fn waitpid(pid: i32) -> Result<i32> {
    let mut status: c_int = 0;
    match unsafe { raw::waitpid(pid as pid_t,
                                &mut status as *mut c_int,
                                0) } {
        -1 => Err(Error::last_os_error()),
        _ => Ok(status as i32)
    }
}

/// Make the underlying file of `dst` refer to the
/// underlying file of `src`. If `dst` is open, it will
/// be closed first. See `dup2(2)` in the UNIX manual
/// pages for details.
pub fn dup2(old: RawFd, new_fd: RawFd) -> Result<()> {
    match unsafe { raw::dup2(old, new_fd) } {
        -1 => Err(Error::last_os_error()),
        _ => Ok(())
    }
}

/// Close a file descriptor.
pub fn close(fd: RawFd) -> Result<()> {
    match unsafe { raw::close(fd) } {
        -1 => { return Err(Error::last_os_error()) },
        _ => Ok(())
    }
}

/// Make a new `File` accessing the underlying file
/// descriptor.
pub fn from_raw_fd(fd: RawFd) -> File {
    unsafe { FromRawFd::from_raw_fd(fd) }
}

/// Make a new pipe. The 0-th side of the resulting array is
/// the read side. The 1-th side is the write side. See
/// `pipe(2)` in the UNIX manual pages for details.
pub fn pipe() -> Result<[RawFd; 2]> {
    let mut pipefds: [RawFd; 2] = [0; 2];
    match unsafe { raw::pipe((&mut pipefds).as_mut_ptr()) } {
        -1 => { return Err(Error::last_os_error()) },
        _ => Ok(pipefds)
    }
}
