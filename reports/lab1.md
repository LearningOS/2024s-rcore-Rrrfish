# rCore-Tutorial-Code-2024S lab1笔记

## 思路
我选择了将`TaskInfo`的信息存放在`TaskControlBlock`内。
```rust
/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// the time that current task was first invoked
    pub start_time: usize,
    /// the task syscall times 
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
}
```
增加了三个`TaskManager`的方法，`get_task_time` `get_syscall_times`和`increase_syscall_time`。
第一次尝试将`increase syscall times`的操作放置在`syscall/mod.rs`里，后来在跑测试的时候发现这样会遗漏掉对`get_time`的系统调用统计。又在`get_time`内部加了`increase syscall times`操作，感觉这样增加了耦合，破坏了抽象，但是实在是想不出好办法了。

## 桶计数
作业文档中写了在内核中用桶计数记录系统调用次数会出现问题，但是我使用桶计数还是通过测试了，不太清楚是为什么。

## 一些细节问题
### 移动语义
我的rust使用还是不够熟练，一开始在增加系统调用次数时竟然犯了非常低级的错误导致测试通不过。
```rust
// task/mod.rs
pub fn increase_syscall_time(&self, id: usize) {
    let mut inner = self.inner.exclusive_access();
    let current = inner.current_task;
    let mut syscall_times = inner.tasks[current].syscall_times;
    syscall_times[id] += 1;
}

// task/task.rs
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    // --snip--
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
}
```
TaskControlBlock实现了 `Copy`和`Clone`Trait，其内部的成员也相应实现该Trait，`let mut syscall_times = inner.tasks[current].syscall_times;`实际上并没有实现移动语义，而是得到了一个数组的副本，原来的桶内数据没有被改变。

### RefCell的可变借用
```rust
pub fn get_task_time(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let time = inner.tasks[current].start_time;
//        drop(inner);
        get_time_ms() - time
}
```
一开始写的时候最后一行是`get_time_ms() - inner.tasks[current].start_time`这样会导致RefCell进行了两次的可变借用，会报错，修改之后就不会出现两次借用重叠的现象了。