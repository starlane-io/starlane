use std::marker::PhantomData;
use crate::space::err::SpaceErr;
use crate::space::loc::Topic;
use crate::space::particle::{Status, StatusDetail};
use tokio::task::LocalKey;

pub struct Tracker<T> where T: Task {
  pub topic: String,
  tx: tokio::sync::mpsc::Sender<OpStateUpdate>,
  watch_rx: tokio::sync::watch::Receiver<OpStateUpdate>,
  phantom: PhantomData<T>
}

impl <T> Tracker <T> where T: Task {

    pub fn new( topic: String ) -> Self {
        let (broadcast_tx, mut broadcast_rx) = tokio::sync::mpsc::channel(10);
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(OpStateUpdate::start());

        tokio::spawn( async move {
            while let Ok(task) = broadcast_rx.recv().await {
                watch_tx.send(task).await;
            }
        });

        Self { topic, tx:broadcast_tx, watch_rx, phantom: Default::default() }
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<OpStateUpdate> {

    }
}

pub trait Task {

}


pub struct OpStateUpdate {
    pub name: String,
    pub description: String,
    pub task: TaskState,
}

impl OpStateUpdate {
    fn start() -> OpStateUpdate {
        Self {
            name: "".to_string(),
            task: Default::default(),
            description: "".to_string(),
        }
    }
}

pub enum TaskState {
    Pending,
    Step(TaskStep),
    Warn(TaskStep,String),
    Error(TaskStep,SpaceErr),
    Fatal(TaskStep,SpaceErr),
    Done(TaskReport)
}

pub struct TaskReport(String);

impl ToString for TaskReport {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}


pub struct TaskStep {
    pub name: String,
    pub step_progression: StepProgression
}

impl Default for TaskStep {
    fn default() -> Self {
        Self {
            name: "Pending".to_string(),
            step_progression:  StepProgression::Pending
        }
    }
}

pub enum StepProgression{
    /// progression always starts with Pending until its state can be quantified
    Pending,
    /// between 0f64..1f64
    Progress(f32),
    /// used when progression can't be quantified by a scalar float
    InProgress,
    /// when complete a StatusDetail must be provided for in the case where
    /// the step may have failed
    Complete(StatusDetail)
}


pub struct TopicSubscriber {
    rx: tokio::sync::watch::Receiver<Tracker>,
}



macro_rules! step{
    ($($arg:tt)*) => {{
        $format!($($arg)*);
    }};
}
