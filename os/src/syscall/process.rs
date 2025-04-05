//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, get_syscall_times, suspend_current_and_run_next},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request  {
        0 => {
            let id: *const u8 = id as *const u8;
            // 读取id地址对应的数据
            let data = unsafe { core::ptr::read_volatile(id) };
            return data as isize;
        },
        1 => {
            let id: *mut u8 = id as *mut u8;
            // 写入data到id地址
            unsafe { core::ptr::write_volatile(id, data as u8) };
            return 0;
        },
        2 => {
            get_syscall_times(id).try_into().unwrap()
        },
        _ => return -1,
    }
}
