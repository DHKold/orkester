mod traits;
mod shell;
mod container;
mod http;
mod kubernetes;
pub mod shell_component;
pub mod http_component;
pub mod container_component;
pub mod kubernetes_component;

// Stream adapter shared across task runner implementations.
pub(crate) mod stream_adapter {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use crossbeam_channel::Receiver;
    use futures_core::Stream;

    pub struct CrossbeamStream<T> {
        rx: Receiver<T>,
    }

    impl<T> CrossbeamStream<T> {
        pub fn new(rx: Receiver<T>) -> Self { Self { rx } }
    }

    impl<T: Unpin> Stream for CrossbeamStream<T> {
        type Item = T;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
            match self.rx.try_recv() {
                Ok(item) => Poll::Ready(Some(item)),
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => Poll::Ready(None),
            }
        }
    }
}

pub use traits::{
    TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream, TaskRunner, TaskRunnerError,
};
pub use shell::ShellTaskRunner;
pub use container::ContainerTaskRunner;
pub use http::HttpTaskRunner;
pub use kubernetes::KubernetesTaskRunner;
pub use shell_component::{ShellTaskRunnerComponent, ShellTaskRunnerConfig};
pub use http_component::{HttpTaskRunnerComponent, HttpTaskRunnerConfig};
pub use container_component::{ContainerTaskRunnerComponent, ContainerTaskRunnerConfig};
pub use kubernetes_component::{KubernetesTaskRunnerComponent, KubernetesTaskRunnerConfig};