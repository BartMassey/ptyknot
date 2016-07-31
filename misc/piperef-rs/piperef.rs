// Copyright © 2016 Bart Massey

// Demonstrate pipes and redirecting child stdout.

extern crate libc;

use std::fs::File;
use std::io::*;
use std::os::unix::io::FromRawFd;

#[macro_use]
mod cmacros;

enum WriteMode {
    #[allow(dead_code)]
    C,
    Macro
}

fn go(write_mode: WriteMode) {
    let mut pipefds = [0; 2];
    check_cint!(libc::pipe((&mut pipefds).as_mut_ptr()));
    let pid: libc::pid_t =  try_cint!(libc::fork());
    if pid == 0 {
        check_cint!(libc::close(pipefds[0]));
        check_cint!(libc::setsid());
        check_cint!(libc::dup2(pipefds[1], 1));
        match write_mode {
            WriteMode::Macro => {println!("hello world");},
            WriteMode::C => {
                let buf = "hello world".as_bytes();
                let hello = std::ffi::CString::new(buf).unwrap();
                let bufptr = hello.as_ptr() as *const libc::c_void;
                check_cint!(libc::write(1, bufptr, buf.len()));
            }
        }
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
    assert_eq!(message.trim(), "hello world");
}


fn main() {
    go(WriteMode::Macro);
}

#[test]
fn write_c_test() {
    go(WriteMode::C);
}


#[test]
fn write_macro_test() {
    main()
}
