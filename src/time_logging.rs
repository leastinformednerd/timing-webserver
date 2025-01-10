use std::collections::BTreeMap;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::spawn::TimingInfo;

const CHANNEL_SIZE: usize = 512;

#[derive(Debug)]
pub enum LoggingMessage {
    ProcessCompleted { token: u32, timing_info: TimingInfo },
    Dump,
}

pub struct TimeLogger {
    rx: mpsc::Receiver<LoggingMessage>,
    // I believe there's a better implementations of maps for ints (a prefix tree?) which I might use
    // https://citeseerx.ist.psu.edu/viewdoc/summary?doi=10.1.1.37.5452
    coalesced: BTreeMap<u32, TimingInfo>,
}

async fn logging_loop(mut tl: TimeLogger) -> () {
    use LoggingMessage::*;
    loop {
        match tl.rx.recv().await {
            Some(msg) => match msg {
                ProcessCompleted { token, timing_info } => {
                    println!("Received {}, {:?}", token, timing_info);
                    tl.coalesced.insert(
                        token,
                        match tl.coalesced.get(&token) {
                            Some(cur_time) => cur_time + timing_info,
                            None => timing_info,
                        },
                    );
                }
                Dump => {
                    println!("{:?}", tl.coalesced);
                }
            },
            None => {}
        }
    }
}

impl TimeLogger {
    pub fn new() -> (JoinHandle<()>, mpsc::Sender<LoggingMessage>) {
        let (tx, rx) = mpsc::channel(CHANNEL_SIZE);

        let tl = TimeLogger {
            rx,
            coalesced: BTreeMap::new(),
        };

        let jh = tokio::spawn(logging_loop(tl));

        (jh, tx)
    }
}
