pub mod output;
pub mod tictactoe;

use libp2p::{
    floodsub::{Floodsub, FloodsubEvent, Topic},
    futures::{StreamExt},
    identity,
    mdns::{Mdns, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, Swarm, SwarmBuilder},
    NetworkBehaviour, PeerId,
};

use tokio::{io::AsyncBufReadExt, sync::mpsc::{self}};
use itertools::Itertools;

pub struct UserSession {
    user_key : identity::Keypair,
    user_peer_id : PeerId,
    game_session : GameSession,
}

impl UserSession {
    pub fn new() -> UserSession {
        let key = identity::Keypair::generate_ed25519();
        UserSession { 
            user_key: key.clone(),
            user_peer_id: PeerId::from(key.public()),
            game_session: GameSession::new(),
         }
    }
}

    pub async fn start<Output: output::PrintToOutput>() {
        let mut user_session = UserSession::new();

        // TODO all in generic output class
        Output::print_string(format!("Your peer id: {:?}", user_session.user_peer_id).as_str());
        Output::print_help();
        let (response_sender, mut response_rcv) = mpsc::unbounded_channel();
        let mut swarm = init_swarm(&user_session, response_sender).await;
        let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

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
                    EventType::GameResponse(game_status) => resolve_spawned_messages::<Output>(game_status, &mut user_session.game_session, &user_session.user_peer_id.to_string()),
                    EventType::Input(line) => match line.as_str() {
                        cmd if cmd.starts_with(output::Commands::Help.to_string()) => Output::print_help(),
                        cmd if cmd.starts_with(output::Commands::Peers.to_string())  => list_peers(&mut swarm).await,
                        cmd if cmd.starts_with(output::Commands::Turn.to_string())  => make_turn::<Output>(&mut swarm, cmd, &mut user_session.game_session).await, 
                        cmd if cmd.starts_with(output::Commands::Start.to_string()) => initiate_game(&mut swarm, cmd, &mut user_session.game_session).await,
                        cmd if cmd == "y" || cmd == "yes" => {
                            send_answer(&mut swarm, &user_session.game_session, true);
                            println!("Waiting for opponent turn.");
                        }
                        cmd if cmd == "n" || cmd == "no" => send_answer(&mut swarm, &user_session.game_session, false),
                        _ => {
                            println!("Unknown command");
                            Output::print_help();
                        }
                    },
                }
            }
        }
    }
    
async fn init_swarm(user_sess : &UserSession, response_sender : tokio::sync::mpsc::UnboundedSender<GameStatus>) -> Swarm<TicTacToeBehaviour> {
    let transport = libp2p::development_transport(user_sess.user_key.clone()).await.expect("transport create failed");
    
        let mut behaviour = TicTacToeBehaviour {
            floodsub: Floodsub::new(user_sess.user_peer_id),
            mdns: Mdns::new(Default::default())
            .await
            .expect("can create mdns"),
            response_sender,
        };
    
    behaviour.floodsub.subscribe(user_sess.game_session.topic.clone());
    let mut swarm = SwarmBuilder::new(transport, behaviour, user_sess.user_peer_id)
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build();
        
             // Tell the swarm to listen on all interfaces and a random, OS-assigned port.
             swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().expect("can get a local socket"),)
             .expect("swarm can be started");
    return swarm;
    }

struct GameSession {
    opponent_id : String,
    game : tictactoe::TicTacToe, 
    topic : Topic,
    your_turn : Option<bool>,
}

impl GameSession {
    fn new() -> GameSession {
        GameSession {
            opponent_id : String::new(),
            game : tictactoe::TicTacToe::new(),
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
        self.your_turn.unwrap_or(false)
    }

    fn make_opponent_turn(&mut self, x: usize, y: usize) {
        self.game.make_opponent_turn(x, y);
        self.your_turn = Some(true);
    }

    fn make_my_turn(&mut self, x: usize, y: usize) -> Result<(), tictactoe::GameError> {
        self.your_turn = Some(false);
        self.game.make_my_turn(x, y)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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

fn resolve_spawned_messages<T : output::PrintToOutput>(game_status : GameStatus, game_session : &mut GameSession, user_peer_id : &str) {
    match game_status {
        GameStatus::Init(initiator_id) => {
            if initiator_id == user_peer_id {
                print!("<{}>: ", initiator_id);
                println!("Do you want to play TicTacToe with me? y[es] or n[o] ?");
                game_session.initiate(initiator_id, false);
            }
        },
        GameStatus::Start => {
            T::print_table(game_session.game.get_state());
            println!("Make turn with command 'turn x y'");
        },
        GameStatus::Turn(x, y) => resolve_opponent_turn::<T>(x, y, game_session),
       };
}

fn resolve_opponent_turn<T : output::PrintToOutput>(x : usize, y: usize, game_session : &mut GameSession) {
    game_session.make_opponent_turn(x, y);
    T::print_table(game_session.game.get_state());

    if game_session.game.is_opponent_winner() {
        println!("Game over, you lose!");
        game_session.reset();
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MyTurn {
    x: usize,
    y: usize,
}

async fn make_turn<Output: output::PrintToOutput>(swarm: &mut Swarm<TicTacToeBehaviour>, line: &str, game_session : &mut GameSession) {
    if game_session.is_your_turn() {

    match Output::process_coords(line) {
        Some((x, y)) => make_one_turn::<Output>(swarm, game_session, x, y).await,
        None => println!("Play again!"),
    };
 } else {
    println!("It is not your turn, waiting for opponent!");
 }
}

async fn make_one_turn<Output : output::PrintToOutput>(swarm: &mut Swarm<TicTacToeBehaviour>, game_session : &mut GameSession, x: usize, y: usize) {
    match game_session.make_my_turn(x, y) {
    
        Ok(()) => {
            Output::print_table(game_session.game.get_state());
        
            if game_session.game.am_i_winner() {
                Output::print_string("Congrats, you win!");
                game_session.reset();
        
            } else {
                Output::print_string("Waiting for opponent turn");
            }
        
            let turn = MyTurn {x, y};
            let json = serde_json::to_string(&turn).expect("cannot jsonify request");
            swarm.behaviour_mut().floodsub.publish(game_session.topic.clone(), json.as_bytes());
        },
    
        Err(tictactoe::GameError::OccupiedField) => Output::print_string("Field is already occupied, choose different one!"),
        Err(tictactoe::GameError::InvalidValue) => Output::print_string("Invalid coordinates, use values in format 'turn <A|B|C> <1|2|3>'"),
    }
}