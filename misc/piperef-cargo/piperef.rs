// Copyright © 2016 Bart Massey

// Demonstrate pipes and redirecting child stdout.

extern crate libc;

use std::ffi::CString;
use std::io::Error;

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
    let mut pipefds : [libc::c_int; 2] = [0, 0];
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
    let r_mode = CString::new("r").unwrap().as_ptr();
    let child_stdout = try_cptr!(libc::fdopen(pipefds[0], r_mode));
    let mut strbuf = [0u8;64];
    check_cptr!(libc::fgets(strbuf.as_mut_ptr() as *mut libc::c_char,
                            strbuf.len() as libc::c_int,
                            child_stdout));
    print!("got {}", std::str::from_utf8(&strbuf).expect("bad string"));
    let mut wstatus = 0;
    check_cint!(libc::waitpid(pid, &mut wstatus as *mut libc::c_int, 0));
    let code = try_cint!(libc::WEXITSTATUS(wstatus));
    if code != 0 {
        panic!("child exited with status {}\n", code);
    }
}
