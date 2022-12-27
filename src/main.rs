#[macro_use]
extern crate quickcheck;

pub mod network_communication;

#[tokio::main]
async fn main() {
    let mut input = network_communication::input::Stdio::new();
    network_communication::start::<network_communication::input::Stdio>(&mut input).await;
}
