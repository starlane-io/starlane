use crate::parse::SkewerCase;
use crate::status::{Status, StatusDetail, StatusWatcher};
use crate::wasm::Timestamp;
use indexmap::IndexMap;

pub type Watcher = tokio::sync::mpsc::Receiver<TaskState>;

pub struct Tracker {
    watcher: Watcher,
}

impl Tracker {
    pub fn states(&self) -> &IndexMap<&'static str, TaskState> {
        todo!()
    }
}

enum TrackerCall {
    State(TaskState),
    TaskStates(tokio::sync::oneshot::Sender<IndexMap<SkewerCase, TaskState>>),
}

struct TrackerRunner {
    rx: tokio::sync::mpsc::Receiver<TaskState>,
    tasks: IndexMap<SkewerCase, TaskState>,
}

impl TrackerRunner {
    fn new() -> tokio::sync::mpsc::Sender<TaskState> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let tasks = IndexMap::new();
        let runner = Self { rx, tasks };

        tokio::spawn(async move {
            runner.run().await;
        });

        tx
    }

    async fn run(mut self) {
        while let Some(state) = self.rx.recv().await {}
    }
}

#[derive(Clone)]
pub struct Progress {
    tx: tokio::sync::mpsc::Sender<TaskState>,
}

impl Progress {
    /// starts a new task
    fn task(&self, task: &'static str) -> impl Task {
        private::Task::new(task, self.tx.clone())
    }
}

pub struct TaskState {
    pub name: &'static str,
    pub step: &'static str,
    /// should be constrained in range 0..100 I guess
    pub inc: u16,
    pub status: Status,
}

impl TaskState {
    pub fn new(name: &'static str, step: &'static str, inc: u16) -> Self {
        let status = Status::default();
        Self {
            name,
            step,
            inc,
            status,
        }
    }
}

pub trait Task {
    /// provide textual updates to the parent progress bar
    fn step(&mut self, step: &'static str);

    /// provide an increment (a number between 0..100)
    /// the task should complete when it reaches 100
    fn inc(&mut self, inc: u16);

    /// end this task and create another
    fn task(self, task: &'static str) -> impl Task;

    fn status(&mut self, status: Status);

    /// end this task
    fn end(self);
}

pub mod private {
    use crate::progress::TaskState;
    use crate::status::{Status, StatusDetail, StatusProbe, StatusWatcher};
    use starlane_space::parse::SkewerCase;

    pub struct Task {
        name: &'static str,
        step: &'static str,
        inc: u16,
        tx: tokio::sync::mpsc::Sender<TaskState>,
        status: Status,
    }

    impl Task {
        pub fn new(name: &'static str, tx: tokio::sync::mpsc::Sender<TaskState>) -> Self {
            let task = name.to_string();
            let status = Status::default();
            let task = Self {
                name,
                step: "started",
                inc: 0u16,
                status,
                tx,
            };

            task.update();
            task
        }
    }

    impl Task {
        fn update(&self) {
            let state = TaskState::new(self.name, self.step, self.inc.clone());
            self.tx.try_send(state).unwrap_or_default();
        }
    }

    impl super::Task for Task {
        fn step(&mut self, step: &'static str) {
            self.step = step;
            self.inc = 0u16;
            self.update()
        }

        fn inc(&mut self, inc: u16) {
            self.inc = inc;
            self.update();
        }

        fn task(self, name: &'static str) -> impl super::Task {
            Task::new(name, self.tx.clone())
        }

        fn status(&mut self, status: Status) {
            self.status = status
        }

        fn end(self) {}
    }

    impl Drop for Task {
        fn drop(&mut self) {
            super::Task::inc(self, 100u16);
            self.update();
        }
    }
}
