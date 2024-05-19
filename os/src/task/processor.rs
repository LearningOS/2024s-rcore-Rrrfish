//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.


use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::config::MAX_SYSCALL_NUM;
use crate::sync::UPSafeCell;
use crate::timer::get_time_ms;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }


    /// mmap
    pub fn mmap(&self, start: usize, len: usize, port: usize) -> isize {
        let current = self.current().unwrap();
        let mut inner = current.inner_exclusive_access();
        inner.memory_set.mmap(start, len, port)
    }

    /// unmmap
    pub fn unmmap(&self, start: usize, len: usize) -> isize {
        let current = self.current().unwrap();
        let mut inner = current.inner_exclusive_access();
        inner.memory_set.munmap(start, len)
    }

    /// translate user address to physical address in current task
    pub fn translate_useraddr(&self, ptr: *const u8) -> usize {
        let current = self.current().unwrap();
        let inner = current.inner_exclusive_access();
        inner.memory_set.translate_useraddr_to_physaddr(ptr)
    }

    /// get the time interval since the task was first invoked
    pub fn get_task_time(&self) -> usize { 
        let current = self.current().unwrap();
        let inner = current.inner_exclusive_access();
        let time = inner.start_time;
        //drop(inner);
        get_time_ms() - time
    }

    /// get the task of syscall times
    pub fn get_syscall_times(&self) -> [u32; MAX_SYSCALL_NUM] {
        let current = self.current().unwrap();
        let inner = current.inner_exclusive_access();
        inner.syscall_times
    }

    /// increase the syscall time of current task
    pub fn increase_syscall_time(&self, id: usize) {
        let current = self.current().unwrap();
        let mut inner = current.inner_exclusive_access();
        inner.syscall_times[id] += 1;
    }
}

/// increase the syscall time of current task
pub fn increase_syscall_time(id: usize) {
    PROCESSOR.exclusive_access().increase_syscall_time(id)
}

/// get the task of syscall times
pub fn get_syscall_times() -> [u32; MAX_SYSCALL_NUM] {
PROCESSOR.exclusive_access().get_syscall_times()
}

/// get the time interval since the task was first invoked
pub fn get_task_time() -> usize {
    PROCESSOR.exclusive_access().get_task_time()
}

 /// translate user address to physical address in current task
 pub fn translate_useraddr(ptr: *const u8) -> usize {
    PROCESSOR.exclusive_access().translate_useraddr(ptr)
 }

/// unmmap
pub fn unmmap(start: usize, len: usize) -> isize {
    PROCESSOR.exclusive_access().unmmap(start, len)
}

/// mmap
pub fn mmap(start: usize, len: usize, port: usize) -> isize {
    PROCESSOR.exclusive_access().mmap(start, len, port)
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            let time_ms_now = get_time_ms();
            if let Some(current) = processor.current() {
                let mut inner = current.inner_exclusive_access();
                if inner.start_time == 0 {
                    inner.start_time = time_ms_now;
                }

            }

            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
