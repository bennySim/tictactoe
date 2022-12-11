use network_communication::output::PrintToOutputStdio;

#[macro_use]
extern crate quickcheck;

pub mod network_communication;

use network_communication::NetworkCommunication;

#[tokio::main]
async fn main() {
    let communication = NetworkCommunication::new();
    communication.start::<PrintToOutputStdio>();
}
