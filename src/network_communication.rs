pub mod input;
pub mod tictactoe;

use libp2p::futures::StreamExt;

use itertools::Itertools;
use tokio::{
    io::AsyncBufReadExt,
    sync::mpsc::{self},
};

pub struct UserSession {
    user_key: libp2p::identity::Keypair,
    user_peer_id: libp2p::PeerId,
    game_session: GameSession,
}

impl UserSession {
    pub fn new() -> UserSession {
        let key = libp2p::identity::Keypair::generate_ed25519();
        UserSession {
            user_key: key.clone(),
            user_peer_id: libp2p::PeerId::from(key.public()),
            game_session: GameSession::new(),
        }
    }
}

pub enum OutputEvents {
    ListPeers(Vec<String>),
    GameProposal(String),
    StartTrue([[char; 3]; 3]),
    StartFalse,
    TurnResolved([[char; 3]; 3]),
    GameOver,
}

pub async fn start<UserInt: input::Input<self::Input, self::OutputEvents>>(user__interface : &mut UserInt) {

    let mut user_session = UserSession::new();

    //Output::print_string(format!("Your peer id: {:?}", user_session.user_peer_id).as_str());
   // Output::print_help();

    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();
    let mut swarm = init_swarm(&user_session, response_sender).await;
    loop {
        tokio::select! {
            // command line message
            input = user__interface.get_input() => process_input::<UserInt>(input, &mut swarm, &mut user_session, user__interface).await,
            // spawned message from internal process
            response = response_rcv.recv() => resolve_spawned_messages::<UserInt>(user__interface, response, &mut user_session.game_session, &user_session.user_peer_id.to_string()),
            _ = swarm.select_next_some() => {},
        };
    }
}
pub type Coordinates = (usize, usize);

pub enum CoordinatesError {
    InvalidFormat,
    InvalidValue,
}

pub enum Input {
    ListPeers,
    Turn(usize, usize),
    InitiateGame(String),
    Yes,
    No,
}

async fn process_input<UserInt: input::Input<self::Input, self::OutputEvents>>(input: Option<self::Input>, swarm : &mut libp2p::swarm::Swarm<TicTacToeBehaviour>, user_session : &mut UserSession
, user_interface : &mut UserInt) {
    match input {
        Some(Input::ListPeers) => { list_peers::<UserInt>(swarm, user_interface).await }
        Some(Input::Turn(x, y)) => { make_turn::<UserInt>(swarm, x, y, &mut user_session.game_session).await }
        Some(Input::InitiateGame(peer_id)) => { initiate_game(swarm, peer_id, &mut user_session.game_session).await }
        Some(Input::Yes) => {
            send_answer::<UserInt>(swarm, &user_session.game_session, true);
        }
        Some(Input::No) => { send_answer::<UserInt>(swarm, &user_session.game_session, false) }
        _ => {
        }
    }
}

async fn init_swarm(user_sess: &UserSession, response_sender: tokio::sync::mpsc::UnboundedSender<GameStatus>) -> libp2p::swarm::Swarm<TicTacToeBehaviour> {
    let transport = libp2p::development_transport(user_sess.user_key.clone())
        .await
        .expect("transport create failed");

    let mut behaviour = TicTacToeBehaviour {
        floodsub: libp2p::floodsub::Floodsub::new(user_sess.user_peer_id),
        mdns: libp2p::mdns::Mdns::new(Default::default())
            .await
            .expect("can create mdns"),
        response_sender,
    };

    behaviour
        .floodsub
        .subscribe(user_sess.game_session.topic.clone());
    let mut swarm = libp2p::swarm::SwarmBuilder::new(transport, behaviour, user_sess.user_peer_id)
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        }))
        .build();

    // Tell the swarm to listen on all interfaces and a random, OS-assigned port.
    swarm
        .listen_on(
            "/ip4/0.0.0.0/tcp/0"
                .parse()
                .expect("can get a local socket"),
        )
        .expect("swarm can be started");
    swarm
}

struct GameSession {
    opponent_id: String,
    game: tictactoe::TicTacToe,
    topic: libp2p::floodsub::Topic,
    your_turn: Option<bool>,
}

impl GameSession {
    fn new() -> GameSession {
        GameSession {
            opponent_id: String::new(),
            game: tictactoe::TicTacToe::new(),
            topic: libp2p::floodsub::Topic::new("TicTacToe"),
            your_turn: None,
        }
    }

    fn initiate(&mut self, opp_id: String, your_turn: bool) {
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

#[derive(Debug)]
enum GameStatus {
    Init(InitiatorId),
    Start(bool),
    Turn(usize, usize),
}

#[derive(libp2p::NetworkBehaviour)]
struct TicTacToeBehaviour {
    floodsub: libp2p::floodsub::Floodsub,
    mdns: libp2p::mdns::Mdns,
    #[behaviour(ignore)]
    response_sender: mpsc::UnboundedSender<GameStatus>,
}

impl libp2p::swarm::NetworkBehaviourEventProcess<libp2p::floodsub::FloodsubEvent>
    for TicTacToeBehaviour
{
    fn inject_event(&mut self, event: libp2p::floodsub::FloodsubEvent) {
        if let libp2p::floodsub::FloodsubEvent::Message(msg) = event {
            if let Ok(resp) = serde_json::from_slice::<Request>(&msg.data) {
                spawn_internally(self.response_sender.clone(), GameStatus::Init(resp.sender));
            }

            if let Ok(resp) = serde_json::from_slice::<Answer>(&msg.data) {
                spawn_internally(self.response_sender.clone(), GameStatus::Start(resp.accept));
            }

            if let Ok(opponent_turn) = serde_json::from_slice::<MyTurn>(&msg.data) {
                spawn_internally(
                    self.response_sender.clone(),
                    GameStatus::Turn(opponent_turn.x, opponent_turn.y),
                );
            }
        }
    }
}

fn spawn_internally(sender: mpsc::UnboundedSender<GameStatus>, game_status: GameStatus) {
    tokio::spawn(async move {
        sender
            .send(game_status)
            .expect("Error while sending message");
    });
}

impl libp2p::swarm::NetworkBehaviourEventProcess<libp2p::mdns::MdnsEvent> for TicTacToeBehaviour {
    fn inject_event(&mut self, event: libp2p::mdns::MdnsEvent) {
        match event {
            libp2p::mdns::MdnsEvent::Discovered(discovered_list) => {
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            libp2p::mdns::MdnsEvent::Expired(expired_list) => {
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

async fn get_peers(swarm: &mut libp2p::swarm::Swarm<TicTacToeBehaviour>) -> Vec<&libp2p::PeerId> {
    let nodes = swarm.behaviour().mdns.discovered_nodes();
    nodes.into_iter().unique().collect()
}

async fn list_peers<Output: input::Input<Input, OutputEvents>>(
    swarm: &mut libp2p::swarm::Swarm<TicTacToeBehaviour>,
    user_interface : &mut Output,
) {
    let peers = get_peers(swarm).await.iter().map(|peerId| peerId.to_string()).collect_vec();
    user_interface.print_to_output(OutputEvents::ListPeers(peers));
    //Output::print_string(format!("Discovered {} peers:", peers.len()).as_str());

    //peers
      //  .iter()
      //  .enumerate()
      //  .for_each(|(i, el)| Output::print_string(format!("{}: {}", i, el).as_str()));
}

fn resolve_spawned_messages<Output: input::Input<Input, OutputEvents>>(
    user_interface : &mut Output,
    game_status: Option<GameStatus>,
    game_session: &mut GameSession,
    user_peer_id: &str,
) {
    match game_status.expect("response exists") {
        GameStatus::Init(initiator_id) => {
            if initiator_id == user_peer_id {
                user_interface.print_to_output(OutputEvents::GameProposal(user_peer_id.to_string()));
                game_session.initiate(initiator_id, false);
            }
        }
        GameStatus::Start(true) => 
            user_interface.print_to_output(OutputEvents::StartTrue(game_session.game.get_state())),
        GameStatus::Start(false) => user_interface.print_to_output(OutputEvents::StartFalse),
        GameStatus::Turn(x, y) => resolve_opponent_turn::<Output>(x, y, game_session, user_interface),
    };
}

fn resolve_opponent_turn<Output: input::Input<Input, OutputEvents>>(
    x: usize,
    y: usize,
    game_session: &mut GameSession,
    user_interface : &mut Output
) {
    game_session.make_opponent_turn(x, y);
    user_interface.print_to_output(OutputEvents::TurnResolved(game_session.game.get_state()));

    if game_session.game.is_opponent_winner() {
        user_interface.print_to_output(OutputEvents::GameOver);
        game_session.reset();
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Answer {
    accept: bool,
}

fn send_answer<Output: input::Input<Input, OutputEvents>>(
    swarm: &mut libp2p::swarm::Swarm<TicTacToeBehaviour>,
    game_session: &GameSession,
    answer: bool,
) {
    if game_session.is_initiated() {
        let answer = Answer { accept: answer };
        let json = serde_json::to_string(&answer).expect("cannot jsonify request");
        swarm
            .behaviour_mut()
            .floodsub
            .publish(game_session.topic.clone(), json.as_bytes());
    } else {
        //Output::print_string("Unknown command");
    }
}

async fn initiate_game(
    swarm: &mut libp2p::swarm::Swarm<TicTacToeBehaviour>,
    peerId: String,
    game_session: &mut GameSession,
) {

            let index: usize = peerId.parse().unwrap(); // TODO handle errors
            let peers = get_peers(swarm).await;
            let receiver_peer_id = peers[index].to_string();
            let req = Request {
                sender: receiver_peer_id.clone(),
            };
            game_session.initiate(receiver_peer_id, true);
            let json = serde_json::to_string(&req).expect("cannot jsonify request");
            swarm
                .behaviour_mut()
                .floodsub
                .publish(game_session.topic.clone(), json.as_bytes());
       
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MyTurn {
    x: usize,
    y: usize,
}

async fn make_turn<Output: input::Input<Input, OutputEvents>>(
    swarm: &mut libp2p::swarm::Swarm<TicTacToeBehaviour>,
    x : usize,
    y : usize,
    game_session: &mut GameSession,
) {
    if game_session.is_your_turn() {
        make_one_turn::<Output>(swarm, game_session, x, y).await;
    } else {
        //Output::print_string("It is not your turn, waiting for opponent!");
    }
}

async fn make_one_turn<Output: input::Input<Input, OutputEvents>>(
    swarm: &mut libp2p::swarm::Swarm<TicTacToeBehaviour>,
    game_session: &mut GameSession,
    x: usize,
    y: usize,
) {
    match game_session.make_my_turn(x, y) {
        Ok(()) => {
            //Output::print_table(game_session.game.get_state());

            if game_session.game.am_i_winner() {
               // Output::print_string("Congrats, you win!");
                game_session.reset();
            } else {
              //  Output::print_string("Waiting for opponent turn");
            }

            let turn = MyTurn { x, y };
            let json = serde_json::to_string(&turn).expect("cannot jsonify request");
            swarm
                .behaviour_mut()
                .floodsub
                .publish(game_session.topic.clone(), json.as_bytes());
        }

        Err(tictactoe::GameError::OccupiedField) => {
            //Output::print_string("Field is already occupied, choose different one!")
        }
        Err(tictactoe::GameError::InvalidValue) => {
            //Output::print_string("Invalid coordinates, use values in format 'turn <A|B|C> <1|2|3>'")
        }
    }
}
