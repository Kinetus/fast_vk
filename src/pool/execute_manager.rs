use super::Message;
use super::Sender;

use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

mod error;
pub use error::ExecuteError;

mod event;
pub use event::Event;
use vk_method::Method;

use super::EventReceiver;
use super::TaskSender;

pub struct ExecuteManager {
    queue: Arc<Mutex<Vec<(Method, Sender)>>>,
    #[allow(dead_code)]
    sender: TaskSender,
    #[allow(dead_code)]
    thread: JoinHandle<()>,
}

impl ExecuteManager {
    pub fn new(
        mut event_receiver: EventReceiver,
        task_sender: TaskSender,
    ) -> ExecuteManager {
        let queue = Arc::new(Mutex::new(Vec::new()));

        let thread_queue = Arc::clone(&queue);
        let sender = task_sender.clone();

        let thread = tokio::spawn(async move {
            loop {
                match event_receiver.recv().await {
                    Some(event) => match event {
                        #[allow(unused_must_use)]
                        Event::FreeWorker => {
                            ExecuteManager::push_execute(&mut thread_queue.lock().unwrap(), &task_sender);
                        }
                    },
                    None => {
                        break;
                    }
                }
            }
        });

        ExecuteManager {
            thread,
            queue,
            sender,
        }
    }

    fn push_execute(queue: &mut Vec<(Method, Sender)>, work_sender: &TaskSender) -> Result<(), anyhow::Error> {
        if queue.len() == 0 {
            return Err(ExecuteError::EmptyQueue.into())
        }

        let methods_len = if queue.len() < 25 { queue.len() } else { 25 };
        let methods_with_senders = queue.drain(0..methods_len);

        let mut methods = Vec::new();
        let mut senders = Vec::new();

        for (method, sender) in methods_with_senders {
            methods.push(method);
            senders.push(sender);
        }

        work_sender
            .send(Message::NewExecute(methods, senders))?;
        
        Ok(())
    }

    pub fn push(&self, method: Method, sender: Sender) -> Result<(), anyhow::Error> {
        let mut queue = self.queue.lock().unwrap();
        queue.push((method, sender));
        
        if queue.len() >= 25 {
            ExecuteManager::push_execute(&mut queue, &self.sender)?;
        }

        Ok(())
    }
}
