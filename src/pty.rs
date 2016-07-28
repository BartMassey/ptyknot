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
use std::io::{Result, Error, ErrorKind};
use std::os::unix::io::{RawFd, AsRawFd};
use std::ffi::{CStr, OsString};
use std::os::unix::ffi::OsStringExt;
use libc::{c_int, pid_t};

mod raw {
    use libc::{c_int, c_char};
    pub use libc::{waitpid, dup2, close};

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
pub fn dup2<T: AsRawFd>(old: &T, new_fd: RawFd) -> Result<()> {
    match unsafe { raw::dup2(old.as_raw_fd(), new_fd) } {
        -1 => Err(Error::last_os_error()),
        _ => Ok(())
    }
}

/// Close the underlying file descriptor of the given
/// object. Subsequent accesses will fail.
pub fn close<T: AsRawFd>(fd: &T) -> Result<()> {
    match unsafe { raw::close(fd.as_raw_fd()) } {
        -1 => { return Err(Error::last_os_error()) },
        _ => Ok(())
    }
}
