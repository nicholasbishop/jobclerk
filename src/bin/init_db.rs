use anyhow::Error;
use fehler::throws;
use tokio_postgres::NoTls;

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

    client
        .batch_execute(include_str!("../../db/init.sql"))
        .await?;
}
