use libp2p::{
    floodsub::{Floodsub, FloodsubEvent, Topic},
    futures::StreamExt,
    identity,
    mdns::{Mdns, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, Swarm, SwarmBuilder},
    NetworkBehaviour, PeerId,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncBufReadExt, sync::mpsc};
use itertools::Itertools;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::tictactoe::TicTacToe;

pub mod tictactoe;

static TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("TicTacToe"));
static USER_KEY : Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
static USER_PEER_ID : Lazy<PeerId> = Lazy::new(|| PeerId::from(USER_KEY.public()));

struct GameSession {
    opponent_id : String,
    game : TicTacToe, 
    initiated : bool,
}

impl GameSession {
    fn new() -> GameSession {
        GameSession {
            opponent_id : String::new(),
            game : TicTacToe::new(),
            initiated : false,
        }
    }

    fn initiate(&mut self, opp_id: String) {
        self.opponent_id = opp_id;
        self.initiated = true;
    }

    fn reset(&mut self) {
        self.game.reset();
        self.initiated = false;
        self.opponent_id = String::new();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    sender: String,
}

type InitiatorId = String;
enum GameStatus {
    Init(InitiatorId),
    Start,
    Turn(usize, usize),
}

enum EventType {
    GameResponse(GameStatus),
    Input(String),
}

#[derive(NetworkBehaviour)]
struct TicTacToeBehaviour {
    floodsub: Floodsub,
    mdns: Mdns,
    #[behaviour(ignore)]
    response_sender: mpsc::UnboundedSender<GameStatus>,
}

impl NetworkBehaviourEventProcess<FloodsubEvent> for TicTacToeBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        if let FloodsubEvent::Message(msg) = event {
            if let Ok(resp) = serde_json::from_slice::<Request>(&msg.data) {
               process_request_spawn(self.response_sender.clone(), resp.sender);
            }

            if let Ok(resp) = serde_json::from_slice::<Answer>(&msg.data) {
                if resp.accept {
                    println!("yes");
                    start_game_spawn(self.response_sender.clone());
                } else {
                    println!("no");
                }
            }

            if let Ok(opponent_turn) = serde_json::from_slice::<MyTurn>(&msg.data) {
                turn_opponent_spawn(self.response_sender.clone(), opponent_turn);
            }
        }
    }
}

fn turn_opponent_spawn(sender: mpsc::UnboundedSender<GameStatus>, opponent_turn : MyTurn) {
    tokio::spawn(async move {
        let resp = GameStatus::Turn(opponent_turn.x, opponent_turn.y);
        if sender.send(resp).is_err() {
            println!("Error while sending message");
        }
    });
}

fn process_request_spawn(sender: mpsc::UnboundedSender<GameStatus>, msg_source : String) {
        tokio::spawn(async move {
            let resp = GameStatus::Init(msg_source);
            if sender.send(resp).is_err() {
                println!("Error while sending message");
            }
        });
}

fn start_game_spawn(sender: mpsc::UnboundedSender<GameStatus>) {
    tokio::spawn( async move {
        if sender.send(GameStatus::Start).is_err() {
            println!("Error while sending message");
        }
    });
}

impl NetworkBehaviourEventProcess<MdnsEvent> for TicTacToeBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

#[derive(Debug, EnumIter)]
pub enum Commands {
    Help,
    Start,
    Peers,
    Turn,
}

impl Commands {
    fn to_string(&self) -> &'static str {
        match self {
            Commands::Help => "help",
            Commands::Start => "start",
            Commands::Peers => "peers",
            Commands::Turn => "turn",
        }
    }

    fn description(&self) -> (&'static str, &'static str) {
        match self {
            Commands::Help => ("help", "prints help."),
            Commands::Start => ("start <peer_index>", "sends peer with index <peer_index> offer to play."),
            Commands::Peers => ("peers", "writes <index> : <peer_id> for all active peers."),
            Commands::Turn => ("turn <row> <col>", "sends turn to opponent"),
        }
    }
}

fn print_help() {
    println!("Available commands: ");

    Commands::iter()
    .map(|comm| comm.description())
    .for_each(|(name, desc)| println!("{:20} - {}", name, desc));
}

fn print_table(grid : [[char; 3]; 3]) {
    println!("  1   2   3");
    println!("A {} | {} | {}", grid[0][0], grid[0][1], grid[0][2]);
    println!("  ---------");
    println!("B {} | {} | {}", grid[1][0], grid[1][1], grid[1][2]);
    println!("  ---------");
    println!("C {} | {} | {}", grid[2][0], grid[2][1], grid[2][2]);
}

#[tokio::main]
async fn main() {
    println!("Your peer id: {:?}", USER_PEER_ID.clone());
    print_help();

    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();

    let transport = libp2p::development_transport(USER_KEY.clone()).await.expect("transport create failed");

    let mut behaviour = TicTacToeBehaviour {
        floodsub: Floodsub::new(USER_PEER_ID.clone()),
        mdns: Mdns::new(Default::default())
        .await
        .expect("can create mdns"),
        response_sender,
    };

    behaviour.floodsub.subscribe(TOPIC.clone());

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    let mut swarm = SwarmBuilder::new(transport, behaviour, USER_PEER_ID.clone())
    .executor(Box::new(|fut| {
        tokio::spawn(fut);
    }))
    .build();

    // Tell the swarm to listen on all interfaces and a random, OS-assigned port.
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().expect("can get a local socket"),)
    .expect("swarm can be started");

    let mut game_session = GameSession::new();
    loop {

        let evt = {
            tokio::select! {
                // command line message
                line = stdin.next_line() => Some(EventType::Input(line.expect("can get line").expect("can read line from stdin"))),
                // spawned message from internal process
                response = response_rcv.recv() => Some(EventType::GameResponse(response.expect("response exists"))),
                _ = swarm.select_next_some() => {
                    None
                },
            }
        };

        if let Some(event) = evt {
            match event {
                EventType::GameResponse(game_status) => resolve_spawned_messages(game_status, &mut game_session),
                EventType::Input(line) => match line.as_str() {
                    cmd if cmd.starts_with(Commands::Help.to_string()) => print_help(),
                    cmd if cmd.starts_with(Commands::Peers.to_string())  => list_peers(&mut swarm).await,
                    cmd if cmd.starts_with(Commands::Turn.to_string())  => make_turn(&mut swarm, cmd, &mut game_session).await, 
                    cmd if cmd.starts_with(Commands::Start.to_string()) => initiate_game(&mut swarm, cmd, &mut game_session).await,
                    cmd if cmd == "y" || cmd == "yes" => {
                        send_answer(&mut swarm, &game_session, true);
                        println!("Waiting for opponent turn.");
                    }
                    cmd if cmd == "n" || cmd == "no" => send_answer(&mut swarm, &game_session, false),
                    _ => {
                        println!("Unknown command");
                        print_help();
                    }
                },
            }
        }
    }
    
}

fn resolve_spawned_messages(game_status : GameStatus, game_session : &mut GameSession) {
    match game_status {
        GameStatus::Init(initiator_id) => {
            if initiator_id == USER_PEER_ID.to_string() {
                print!("<{}>: ", initiator_id);
                println!("Do you want to play TicTacToe with me? y[es] or n[o] ?");
                game_session.initiate(initiator_id);
            }
        },
        GameStatus::Start => {
            print_table(game_session.game.get_state());
            println!("Make turn with command 'turn x y'");
        },
        GameStatus::Turn(x, y) => resolve_opponent_turn(x, y, game_session),
       };
}

fn resolve_opponent_turn(x : usize, y: usize, game_session : &mut GameSession) {
    game_session.game.make_opponent_turn(x, y);
    print_table(game_session.game.get_state());

    if game_session.game.is_opponent_winner() {
        println!("Game over, you lose!");
        game_session.reset();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Answer {
    accept: bool,
}

fn send_answer(swarm: &mut Swarm<TicTacToeBehaviour>, game_session : &GameSession, answer : bool) {
    if game_session.initiated {
             let answer = Answer { accept: answer};
             let json = serde_json::to_string(&answer).expect("cannot jsonify request");
             swarm.behaviour_mut().floodsub.publish(TOPIC.clone(), json.as_bytes());
    } else {
     println!("Unknown command");
    }
 }


async fn initiate_game(swarm: &mut Swarm<TicTacToeBehaviour>, line: &str, game_session : &mut GameSession) {
    let rest = line.strip_prefix("start ");
    match rest { // TODO better recognition (strip white...)
        Some("any") => {
            // TODO fill and add as default
        }
        Some(peer_index) => {
            let index: usize = peer_index.parse().unwrap(); // TODO handle errors
            let peers = get_peers(swarm).await;
            let receiver_peer_id = peers[index].to_string();
            let req = Request {
                sender: receiver_peer_id.clone(),
            };
            game_session.initiate(receiver_peer_id);
            let json = serde_json::to_string(&req).expect("cannot jsonify request");
            swarm.behaviour_mut().floodsub.publish(TOPIC.clone(), json.as_bytes());
        }
        None => {// TODO invalid input
    }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MyTurn {
    x: usize,
    y: usize,
}

async fn make_turn(swarm: &mut Swarm<TicTacToeBehaviour>, line: &str, game_session : &mut GameSession) {
    let rest = line.strip_prefix("turn ");
    let coords : Vec<&str> = rest.unwrap().split_whitespace().collect();
    let x = coords[0].parse::<char>().unwrap();
    let y = coords[1].parse::<u8>().unwrap();
    let (x, y) = convert_coords(x, y);
    game_session.game.make_my_turn(x, y);
    print_table(game_session.game.get_state());

    if game_session.game.am_i_winner() {
        println!("Congrats, you win!");
        game_session.reset();
    } else {
        println!("Waiting for opponent turn");
    }
    let turn = MyTurn {x, y};
    let json = serde_json::to_string(&turn).expect("cannot jsonify request");
    swarm.behaviour_mut().floodsub.publish(TOPIC.clone(), json.as_bytes());
}

fn convert_coords(x: char, y: u8) -> (usize, usize) {
    let x = match x {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        _ => 100, // TODO solve when error handling 
    };

    (x, (y-1) as usize)
}

async fn get_peers(swarm: &mut Swarm<TicTacToeBehaviour>) -> Vec<&PeerId> {
    let nodes = swarm.behaviour().mdns.discovered_nodes();
    nodes.into_iter().unique().collect()
}

async fn list_peers(swarm: &mut Swarm<TicTacToeBehaviour>) {
    let peers = get_peers(swarm).await;
    println!("Discovered {} peers:", peers.len());

    peers
    .iter()
    .enumerate()
    .for_each(|(i, el)| println!("{}: {}", i, el));
}
