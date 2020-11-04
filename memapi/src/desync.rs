use flume;
use flume::Sender;
use std::thread;

type Job<Data> = Box<dyn FnOnce(&mut Data) + Send + 'static>;

/// Like the desync Cargo package, only hopefully (1) faster and (2) without
/// unsafe.
#[derive(Debug)]
pub struct Desync<Data>
where
    Data: Send,
{
    sender: Sender<Job<Data>>,
}

impl<Data: Send + 'static> Desync<Data> {
    /// Create a new Desync, which will store the given data in the thread.
    pub fn new(mut data: Data) -> Self {
        let (sender, receiver) = flume::unbounded();
        let desync = Desync { sender };
        thread::spawn(move || loop {
            let job = receiver.recv().unwrap();
            job(&mut data);
        });
        desync
    }

    /// Run a function in the thread, with the data as mutable argument.
    pub fn desync<F>(&self, f: F)
    where
        F: FnOnce(&mut Data) + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).unwrap();
    }

    /// Run a function in the thread, with the data as mutable argument, return
    /// the result to sync()'s caller.
    pub fn sync<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Data) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (sender, receiver) = flume::bounded(1);
        self.desync(move |data| {
            let result = f(data);
            sender.send(result).unwrap();
        });
        receiver.recv().unwrap()
    }
}
