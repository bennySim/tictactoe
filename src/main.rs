use libp2p::{
    floodsub::{Floodsub, FloodsubEvent, Topic},
    futures::StreamExt,
    identity,
    mdns::{Mdns, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, Swarm, SwarmBuilder},
    NetworkBehaviour, PeerId,
};
use serde::{Deserialize, Serialize};
use tictactoe::{GameError, TicTacToe};
use tokio::{io::AsyncBufReadExt, sync::mpsc};
use itertools::Itertools;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub mod tictactoe;

type Coordinates = (usize, usize);

struct NetworkCommunication {
    user_key : identity::Keypair,
    user_peer_id : PeerId,
    game_session : GameSession,
}

impl NetworkCommunication {
    fn new() -> NetworkCommunication {
        let key = identity::Keypair::generate_ed25519();
        NetworkCommunication { 
            user_key: key.clone(),
            user_peer_id: PeerId::from(key.public()),
            game_session: GameSession::new(),
         }
    }
}

struct GameSession {
    opponent_id : String,
    game : TicTacToe, 
    topic : Topic,
    your_turn : Option<bool>,
}

impl GameSession {
    fn new() -> GameSession {
        GameSession {
            opponent_id : String::new(),
            game : TicTacToe::new(),
            topic: Topic::new("TicTacToe"),
            your_turn : None,
        }
    }

    fn initiate(&mut self, opp_id: String, your_turn : bool) {
        self.opponent_id = opp_id;
        self.your_turn = Some(your_turn);
    }

    fn is_initiated(&self) -> bool {
        self.your_turn.is_some()
    }

    fn reset(&mut self) {
        self.game.reset();
        self.opponent_id = String::new();
    }

    fn is_your_turn(&self) -> bool {
        self.your_turn.is_some() && self.your_turn.unwrap()
    }

    fn make_opponent_turn(&mut self, x: usize, y: usize) {
        self.game.make_opponent_turn(x, y);
        self.your_turn = Some(true);
    }

    fn make_my_turn(&mut self, x: usize, y: usize) -> Result<(), GameError> {
        self.your_turn = Some(false);
        self.game.make_my_turn(x, y)
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

enum CoordinatesError {
    InvalidFormat,
    InvalidValue,
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
               spawn_internally(self.response_sender.clone(), GameStatus::Init(resp.sender));
            }

            if let Ok(resp) = serde_json::from_slice::<Answer>(&msg.data) {
                if resp.accept {
                    println!("yes");
                    spawn_internally(self.response_sender.clone(), GameStatus::Start);
                } else {
                    println!("no");
                }
            }

            if let Ok(opponent_turn) = serde_json::from_slice::<MyTurn>(&msg.data) {
                spawn_internally(self.response_sender.clone(), GameStatus::Turn(opponent_turn.x, opponent_turn.y));
            }
        }
    }
}

fn spawn_internally(sender: mpsc::UnboundedSender<GameStatus>, game_status : GameStatus) {
    tokio::spawn(async move {
        if sender.send(game_status).is_err() {
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
    let mut communication = NetworkCommunication::new();
    println!("Your peer id: {:?}", communication.user_peer_id);
    print_help();

    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();

    let transport = libp2p::development_transport(communication.user_key.clone()).await.expect("transport create failed");

    let mut behaviour = TicTacToeBehaviour {
        floodsub: Floodsub::new(communication.user_peer_id),
        mdns: Mdns::new(Default::default())
        .await
        .expect("can create mdns"),
        response_sender,
    };

    behaviour.floodsub.subscribe(communication.game_session.topic.clone());

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    let mut swarm = SwarmBuilder::new(transport, behaviour, communication.user_peer_id)
    .executor(Box::new(|fut| {
        tokio::spawn(fut);
    }))
    .build();

    // Tell the swarm to listen on all interfaces and a random, OS-assigned port.
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().expect("can get a local socket"),)
    .expect("swarm can be started");

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
                EventType::GameResponse(game_status) => resolve_spawned_messages(game_status, &mut communication.game_session, &communication.user_peer_id.to_string()),
                EventType::Input(line) => match line.as_str() {
                    cmd if cmd.starts_with(Commands::Help.to_string()) => print_help(),
                    cmd if cmd.starts_with(Commands::Peers.to_string())  => list_peers(&mut swarm).await,
                    cmd if cmd.starts_with(Commands::Turn.to_string())  => make_turn(&mut swarm, cmd, &mut communication.game_session).await, 
                    cmd if cmd.starts_with(Commands::Start.to_string()) => initiate_game(&mut swarm, cmd, &mut communication.game_session).await,
                    cmd if cmd == "y" || cmd == "yes" => {
                        send_answer(&mut swarm, &communication.game_session, true);
                        println!("Waiting for opponent turn.");
                    }
                    cmd if cmd == "n" || cmd == "no" => send_answer(&mut swarm, &communication.game_session, false),
                    _ => {
                        println!("Unknown command");
                        print_help();
                    }
                },
            }
        }
    }
    
}

fn resolve_spawned_messages(game_status : GameStatus, game_session : &mut GameSession, user_peer_id : &str) {
    match game_status {
        GameStatus::Init(initiator_id) => {
            if initiator_id == user_peer_id {
                print!("<{}>: ", initiator_id);
                println!("Do you want to play TicTacToe with me? y[es] or n[o] ?");
                game_session.initiate(initiator_id, false);
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
    game_session.make_opponent_turn(x, y);
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
    if game_session.is_initiated() {
             let answer = Answer { accept: answer};
             let json = serde_json::to_string(&answer).expect("cannot jsonify request");
             swarm.behaviour_mut().floodsub.publish(game_session.topic.clone(), json.as_bytes());
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
            game_session.initiate(receiver_peer_id, true);
            let json = serde_json::to_string(&req).expect("cannot jsonify request");
            swarm.behaviour_mut().floodsub.publish(game_session.topic.clone(), json.as_bytes());
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
    if game_session.is_your_turn() {

    match process_coords(line) {
        Some((x, y)) => make_one_turn(swarm, game_session, x, y).await,
        None => println!("Play again!"),
    };
 } else {
    println!("It is not your turn, waiting for opponent!");
 }
}

async fn make_one_turn(swarm: &mut Swarm<TicTacToeBehaviour>, game_session : &mut GameSession, x: usize, y: usize) {
    match game_session.make_my_turn(x, y) {
    
        Ok(()) => {
            print_table(game_session.game.get_state());
        
            if game_session.game.am_i_winner() {
                println!("Congrats, you win!");
                game_session.reset();
        
            } else {
                println!("Waiting for opponent turn");
            }
        
            let turn = MyTurn {x, y};
            let json = serde_json::to_string(&turn).expect("cannot jsonify request");
            swarm.behaviour_mut().floodsub.publish(game_session.topic.clone(), json.as_bytes());
        },
    
        Err(GameError::OccupiedField) => println!("Field is already occupied, choose different one!"),
        Err(GameError::InvalidValue) => println!("Invalid coordinates, use values in format 'turn <A|B|C> <1|2|3>'"),
    }
}

fn parse_coords(line: &str) -> Result<Coordinates, CoordinatesError> {
    let rest = line.strip_prefix("turn ");
    let coords : Vec<&str> = rest.unwrap().split_whitespace().collect();

    if coords.len() != 2 {
        return Err(CoordinatesError::InvalidFormat);
    }
    
    let x = coords[0].parse::<char>();
    let y = coords[1].parse::<usize>();
    
    match (x, y) {
        (Ok(x), Ok(y)) => convert_coords(x, y).ok_or(CoordinatesError::InvalidValue),
        (_, _) => Err(CoordinatesError::InvalidFormat)
    }
}

fn process_coords(line: &str) -> Option<Coordinates> {
    match parse_coords(line) {
        Ok(coords) => Some(coords),
        Err(CoordinatesError::InvalidFormat) => { 
            println!("Invalid format, use format 'turn <A|B|C> <1|2|3>'");
            None
        },
        Err(CoordinatesError::InvalidValue) => {
            println!("Invalid range, use values in format 'turn <A|B|C> <1|2|3>'");
            None
        },
    }
    
}

fn convert_coords(x: char, y: usize) -> Option<Coordinates> {
    let x= match x {
        'A' => Some(0),
        'B' => Some(1),
        'C' => Some(2),
        _ => None,
    };

    if x.is_none() || !(1..=3).contains(&y)  {
        None
    } else {
        Some((x.unwrap(), y-1))
    }
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
