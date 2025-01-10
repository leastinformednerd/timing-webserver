use std::{io, process::Stdio};

use tokio::{process::Command, task::JoinSet};

mod spawn;
mod time_logging;

#[tokio::main]
async fn main() -> io::Result<()> {
    let c = |t| {
        spawn::timing_spawn(
            {
                let mut cmd = Command::new("factor");
                // Effectively randomly chosen number. It runs fast but not instantly on my laptop
                cmd.args(["1239223920932090000001"]).stdout(Stdio::null());
                cmd
            },
            t,
        )
    };

    let (_tl, tx) = time_logging::TimeLogger::new();

    let mut join_set = JoinSet::new();

    for i in (0..10).map(|i| i / 3) {
        join_set.spawn(c(i)?);
    }

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok((token, timing_info))) => {
                if tx
                    .send(time_logging::LoggingMessage::ProcessCompleted { token, timing_info })
                    .await
                    .is_err()
                {
                    println!("Sending across channel failed")
                }
            }
            _ => {}
        }
    }

    let _ = tx.send(time_logging::LoggingMessage::Dump).await;

    Ok(())
}
