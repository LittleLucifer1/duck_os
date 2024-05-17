use alloc::{collections::VecDeque, sync::Arc};

use crate::sync::SpinLock;

use super::{pcb::PCB, ORIGIN_TASK};

pub static SCHEDULE: SpinLock<Schedule> = SpinLock::new(Schedule::empty());

// TODO：先采用最简单的RR策略
pub struct Schedule {
    pub task_queue: VecDeque<Arc<PCB>>,
}

impl Schedule {
    pub const fn empty() -> Self {
        Self { task_queue: VecDeque::new(), }
    }

    pub fn size(&self) -> usize {
        self.task_queue.len()
    }
}

pub fn init_schedule() {
    SCHEDULE.lock().task_queue.push_back(Arc::clone(unsafe {ORIGIN_TASK.as_ref().unwrap()}));
}

pub fn push_task_to_schedule(pcb: Arc<PCB>) {
    SCHEDULE.lock().task_queue.push_back(pcb);
}

// TODO: 这里先不考虑测试用例的问题，先简单处理
pub fn pop_task_from_schedule() -> Option<Arc<PCB>> {
    SCHEDULE.lock().task_queue.pop_front()
}

pub fn is_empty() -> bool {
    let schedule_lock = SCHEDULE.lock();
    schedule_lock.task_queue.is_empty()
}