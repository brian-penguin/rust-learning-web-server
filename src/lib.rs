use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    // this takes a similar signature to thread::spawn
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.send(Message::NewJob(job)).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        println!("Sending Terminate message to all workers");

        // Break all the threads out of their loops
        for _ in &mut self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        // Join the threads and resolve! After we're sure we sent terminate to all of them
        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            };
        }
    }
}

// This trait and it's implementation are to trick the rust compiler into being okay with us
// moving a closure out of it's box and calling it as self. I don't understand quite why it doesn't
// just work with us calling (*job)() below but rust is a work in progress
trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Job = Box<dyn FnBox + Send + 'static>;

enum Message {
    NewJob(Job),
    Terminate,
}

struct Worker {
    id: usize,
    // using option here allows drop implementation on threadpool to take the thread from the
    // worker and shut it down gracefully!
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        // Move here allows the thread to take ownership of the receiver
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().unwrap().recv().unwrap();

                match message {
                    Message::NewJob(job) => {
                        println!("Worker {} got a job; executing now.", id);

                        job.call_box();
                    },
                    Message::Terminate => {
                        println!("Worker {} received terminate message", id);
                        break;
                    }
                }
            }
        });

        Worker { id, thread: Some(thread) }
    }
}
