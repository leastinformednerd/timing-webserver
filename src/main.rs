use std::{io, process::Stdio};

use tokio::{process::Command, try_join};

mod spawn;

#[tokio::main]
async fn main() -> io::Result<()> {
    let c = || {
        spawn::timing_spawn({
            let mut cmd = Command::new("factor");
            // This number was chosen arbitarily, it's not perfect (it takes 16 seconds on my laptop)
            // I used to have a 2 second long computation but I lost the number that it was
            cmd.args(["12392239209320900000001"]).stdout(Stdio::null());
            cmd
        })
    };

    println!("Awaiting");
    let fs = try_join!(c()?, c()?, c()?, c()?, c()?, c()?, c()?, c()?,);

    println!("{:?}", fs);

    Ok(())
}
