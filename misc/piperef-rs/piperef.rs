// Copyright © 2016 Bart Massey

// Demonstrate pipes and redirecting child stdout.

extern crate libc;

use std::fs::File;
use std::io::{Error, BufReader, BufRead};
use std::os::unix::io::FromRawFd;

macro_rules! check_cint {
    ($cexp: expr) => (match unsafe { $cexp } {
        -1 => { panic!(Error::last_os_error()); },
        _ => ()
    })
}

macro_rules! try_cint {
    ($cexp: expr) => (match unsafe { $cexp } {
        -1 => { panic!(Error::last_os_error()); },
        r => r
    })
}

macro_rules! check_cptr {
    ($cexp: expr) => ({
        let p = unsafe { $cexp };
        if p.is_null() { panic!(Error::last_os_error()); };
    })
}

macro_rules! try_cptr {
    ($cexp: expr) => ({
        let p = unsafe { $cexp };
        if p.is_null() { panic!(Error::last_os_error()); };
        p
    })
}

fn main() {
    let mut pipefds = [0; 2];
    check_cint!(libc::pipe((&mut pipefds).as_mut_ptr()));
    let pid: libc::pid_t =  try_cint!(libc::fork());
    if pid == 0 {
        check_cint!(libc::close(pipefds[0]));
        check_cint!(libc::setsid());
        check_cint!(libc::dup2(pipefds[1], 1));
        println!("hello world");
        check_cint!(libc::exit(0));
    }
    check_cint!(libc::close(pipefds[1]));
    let child_stdout: File = unsafe { FromRawFd::from_raw_fd(pipefds[0]) };
    let mut message = String::new();
    let mut reader = BufReader::new(child_stdout);
    reader.read_line(&mut message)
          .expect("could not read message");
    let mut wstatus = 0;
    check_cint!(libc::waitpid(pid, &mut wstatus as *mut libc::c_int, 0));
    let code = try_cint!(libc::WEXITSTATUS(wstatus));
    if code != 0 {
        panic!("child exited with status {}\n", code);
    }
    print!("got {}", message);
}

