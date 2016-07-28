// Copyright © 2016 Bart Massey
// Macros for calling C code.

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
