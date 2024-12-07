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
    pub name: String,
    pub step: Option<String>,
    pub inc: u16,
}

impl TaskState {
    pub fn new(name: String, step: Option<String>, inc: u16) -> Self {
        Self { name, step, inc }
    }
}

pub trait Task {
    /// provide textual updates to the parent progress bar
    fn step(&mut self, task: impl AsRef<str>);

    /// provide an increment (a number between 0..100)
    /// the task should complete when it reaches 100
    fn inc(&mut self, inc: u16);

    /// end this task and create another
    fn task(self, task: &'static str) -> impl Task;

    /// end this task
    fn end(self);
}

pub mod private {
    use crate::progress::TaskState;

    pub struct Task {
        task: String,
        step: Option<String>,
        inc: u16,
        tx: tokio::sync::mpsc::Sender<TaskState>,
    }

    impl Task {
        pub fn new(task: &'static str, tx: tokio::sync::mpsc::Sender<TaskState>) -> Self {
            let task = task.to_string();
            let task = Self {
                task,
                step: None,
                inc: 0,
                tx,
            };

            task.update();
            task
        }
    }

    impl Task {
        fn update(&self) {
            let state = TaskState::new(self.task.clone(), self.step.clone(), self.inc.clone());
            self.tx.try_send(state).unwrap_or_default();
        }
    }

    impl super::Task for Task {
        fn step(&mut self, step: impl AsRef<str>) {
            self.step = Some(step.as_ref().to_string());
            self.update()
        }

        fn inc(&mut self, inc: u16) {
            self.inc = inc;
            self.update()
        }

        fn task(self, task: &'static str) -> impl super::Task {
            Task::new(task, self.tx.clone())
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
