use anyhow::Error;
use argh::FromArgs;
use fehler::{throw, throws};
use std::fmt;
use std::str::FromStr;
use tokio_postgres::NoTls;

/// Database control.
#[derive(FromArgs)]
struct Opt {
    #[argh(positional)]
    command: Command,
}

#[derive(Debug, PartialEq)]
enum Command {
    Init,
    Clean,
    Test,
}

impl FromStr for Command {
    type Err = &'static str;

    #[throws(Self::Err)]
    fn from_str(s: &str) -> Self {
        if s == "init" {
            Self::Init
        } else if s == "clean" {
            Self::Clean
        } else if s == "test" {
            Self::Test
        } else {
            throw!("invalid command")
        }
    }
}

impl fmt::Display for Command {
    #[throws(fmt::Error)]
    fn fmt(&self, f: &mut fmt::Formatter) {
        let s = match self {
            Self::Init => "init",
            Self::Clean => "clean",
            Self::Test => "test",
        };
        write!(f, "{}", s)?
    }
}

#[throws]
#[tokio::main]
async fn main() {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=postgres", NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let opt: Opt = argh::from_env();

    match opt.command {
        Command::Init => {
            client
                .batch_execute(include_str!("../../../db/init.sql"))
                .await?;
        }
        Command::Clean => {
            client
                .batch_execute(include_str!("../../../db/clean.sql"))
                .await?;
        }
        Command::Test => {
            client
                .batch_execute(include_str!("../../../db/test.sql"))
                .await?;
        }
    }
}
