


pub type ActivityWatcher =  tokio::sync::watch::Receiver<()>;
pub type ActivityReporter=  tokio::sync::watch::Sender<()>;

pub fn activity_reporter() -> ActivityReporter {
   let (reporter,_) = tokio::sync::watch::channel(());
    reporter
}
