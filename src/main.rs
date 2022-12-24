#[macro_use]
extern crate quickcheck;

pub mod network_communication;

#[tokio::main]
async fn main() {
    network_communication::start::<network_communication::output::PrintToOutputStdio>().await;
}
