//! Copyright © 2016 Bart Massey
//!
//! Start a child process running a specified action, with
//! a new pseudo-tty as its controlling terminal. Give the
//! caller the master side of that terminal for manipulation,
//! along with the process ID of the child. The caller can
//! then later wait for the child to exit.
//!
//! This work is made available under the "MIT License".
//! Please see the file COPYING in this distribution
//! for license terms.

extern crate libc;

use std::fs::{OpenOptions, File};
use std::io::{Result, Error};

#[cfg(test)]
use std::io::BufReader;
#[cfg(test)]
use std::io::prelude::*;

// Most code borrowed from https://github.com/stemjail/tty-rs .
mod pty {
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
}

fn _ptyknot<F: Fn()>(action: F) -> Result<Option<(File, i32)>> {
    let mut master = OpenOptions::new()
                 .read(true).write(true)
                 .open("/dev/ptmx").expect("cannot open ptmx");
    pty::grantpt(&mut master).expect("could not grant pty");
    pty::unlockpt(&mut master).expect("could not unlock pty");
    let pid = unsafe{ libc::fork() };
    match pid {
        -1 => {
            panic!("fork failed: {}", Error::last_os_error());
        },
        0 => {
            let slave_name = pty::ptsname(&mut master)
                             .expect("cannot get pty name");
            drop(master);
            // Thanks much to
            // https://www.win.tue.nl/~aeb/linux/lk/lk-10.html
            // at "Getting a controlling tty" for helping
            // understand this mess.
            if unsafe { libc::setsid() } < 0 {
                panic!("setsid failed");
            }
            let slave = OpenOptions::new()
                        .read(true).write(true)
                        .open(slave_name).expect("cannot open pty");
            drop(slave);
            action();
            Ok(None)
        },
        _ => Ok(Some((master, pid)))
    }
}

/// Start a child process running the given action, with
/// a controlling tty attached to a pseudo-terminal. Returns
/// the master side of the pseudo-terminal as a file, plus
/// the process ID of the child process.
///
/// # Examples
///
/// ```
/// use std::fs::OpenOptions;
/// use std::io::{Write, BufRead, BufReader};
///
/// fn slave() {
///     let mut tty = OpenOptions::new()
///                   .write(true)
///                   .open("/dev/tty")
///                   .expect("cannot open /dev/tty");
///     tty.write("hello world\n".as_bytes())
///        .expect("cannot write to /dev/tty");
///     tty.flush().expect("cannot flush /dev/tty");
/// }
/// 
/// let (master, pid) = ptyknot::ptyknot(slave)
///                     .expect("cannot create slave");
/// let mut master_buf = BufReader::new(&master);
/// let mut message = String::new();
/// master_buf.read_line(&mut message)
///           .expect("could not read message");
/// ```
pub fn ptyknot<F: Fn()>(action: F) -> Result<(File, i32)> {
    match _ptyknot(action) {
        Ok(Some(r)) => Ok(r),
        Ok(None) => panic!("internal error: _ptyknot returned None"),
        Err(e) => Err(e)
    }
}

/// Wait for the specified `ptyknot()` child process
/// to exit, then return the exit status.
///
/// # Examples
///
/// ```rust,no_run
/// let (_, pid) = ptyknot::ptyknot({||()}).expect("cannot create slave");
/// assert_eq!(ptyknot::reap(pid).unwrap(), 0);
/// ```
pub use pty::waitpid as reap;

#[cfg(test)]
fn slave() {
    let mut tty = OpenOptions::new()
                  .write(true)
                  .open("/dev/tty")
                  .expect("cannot open /dev/tty");
    tty.write("hello world\n".as_bytes())
       .expect("cannot write to /dev/tty");
    tty.flush().expect("cannot flush /dev/tty");
}

#[test]
fn it_works() {
    let (master, pid) = ptyknot(slave).expect("ptyknot fail");
    let mut master_buf = BufReader::new(&master);
    let mut message = String::new();
    master_buf.read_line(&mut message)
              .expect("could not read message");
    assert!(message.trim() == "hello world");
    let status = reap(pid).expect("could not reap child");
    assert_eq!(status, 0);
}