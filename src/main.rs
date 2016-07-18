extern crate libc;

use std::io::prelude::*;
use std::fs::OpenOptions;
use std::io::{Error, BufReader};

// Borrowed from https://github.com/stemjail/tty-rs
mod pty {
    use std::path::*;
    use std::io::{Result, Error};
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

    // getcwd implementation used as reference
    pub fn ptsname<T>(master: &mut T) -> Result<PathBuf> where T: AsRawFd {
        let ptr = unsafe { raw::ptsname(master.as_raw_fd()) };
        if ptr.is_null() {
            return Err(Error::last_os_error());
        }
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let buf = cstr.to_str().expect("pathname not utf8");
        let os_string = OsString::from_vec(buf.as_bytes().to_vec());
        Ok(PathBuf::from(os_string))
    }

    pub fn waitpid(pid: pid_t) -> Result<i32> {
        let mut status: c_int = 0;
        match unsafe { raw_waitpid(pid, &mut status as *mut c_int, 0) } {
            -1 => Err(Error::last_os_error()),
            _ => Ok(status as i32)
        }
    }
}

pub fn main() {
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
            // https://www.win.tue.nl/~aeb/linux/lk/lk-10.html
            // at "Getting a controlling tty"
            if unsafe { libc::setsid() } < 0 {
                panic!("setsid failed");
            }
            let mut slave = OpenOptions::new()
                            .read(true).write(true)
                            .open(slave_name).expect("cannot open pty");
            slave.write("hello world\n".as_bytes())
                 .expect("cannot write to /dev/pty");
            slave.flush().expect("cannot flush /dev/pty");
        },
        _ => {
            let mut master_buf = BufReader::new(&master);
            let mut message = String::new();
            master_buf.read_line(&mut message)
                      .expect("could not read message");
            assert!(message.trim() == "hello world");
            assert_eq!(pty::waitpid(pid).expect("child exited abnormally"), 0);
        }
    }
}
