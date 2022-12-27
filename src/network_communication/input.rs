use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use async_trait::async_trait;
use tokio::io::AsyncBufReadExt;

#[async_trait]
pub trait Input<InputType, OutputType> {
    async fn get_input(&mut self) -> Option<InputType>;
    fn print_to_output(&self, outputType : OutputType);
}

pub struct Stdio {
    stdin : tokio::io::BufReader<tokio::io::Stdin>
}

#[async_trait]
impl Input<crate::network_communication::Input, crate::network_communication::OutputEvents> for Stdio {
    async fn get_input(&mut self) -> Option<crate::network_communication::Input> {
        let s = &mut self.stdin;
        let line = s.lines().next_line().await.expect("can get line").expect("can read line from stdin");
        Self::process_input(line.as_str())
    }

    fn print_to_output(&self, outputType : crate::network_communication::OutputEvents) {
        match outputType {
    super::OutputEvents::ListPeers(peers) => {
        std::println!("Discovered {} peers.", peers.len());
        peers.iter().enumerate().for_each(|(i, peer)| println!("{}: {}", i, peer));
    },
    super::OutputEvents::GameProposal(peer_id) => {
        println!("<{}>: Do you want to play TicTacToe with me? y[es] or n[o] ?", peer_id);
    }
    super::OutputEvents::StartTrue(grid) => {
        Self::print_table(grid);
        println!("Make turn with command 'turn x y'");
    },
    super::OutputEvents::StartFalse => {
        println!("No.");
    },
    super::OutputEvents::TurnResolved(grid) => {
        Self::print_table(grid);
        println!("your turn");
    },
    super::OutputEvents::GameOver => println!("You lose, game over!"),
}
    }
}

impl Stdio {
    pub fn new() -> Self {
        Stdio { stdin: tokio::io::BufReader::new(tokio::io::stdin()) }
    }

    fn print_table(grid : [[char; 3]; 3]) {
        println!("  1   2   3");
        println!("A {} | {} | {}", grid[0][0], grid[0][1], grid[0][2]);
        println!("  ---------");
        println!("B {} | {} | {}", grid[1][0], grid[1][1], grid[1][2]);
        println!("  ---------");
        println!("C {} | {} | {}", grid[2][0], grid[2][1], grid[2][2]);
    }

    fn print_string(text: &str) {
        println!("{}", text);
    }

    fn print_help() {
        println!("Available commands: ");
    
        Commands::iter()
        .map(|comm| comm.description())
        .for_each(|(name, desc)| println!("{:20} - {}", name, desc));
    }

    /*fn process_coords(line: &str) -> Option<crate::network_communication::Coordinates> {
        match parse_coords(line) {
            Ok(coords) => Some(coords),
            Err(crate::network_communication::CoordinatesError::InvalidFormat) => { 
                println!("Invalid format, use format 'turn <A|B|C> <1|2|3>'");
                None
            },
            Err(crate::network_communication::CoordinatesError::InvalidValue) => {
                println!("Invalid range, use values in format 'turn <A|B|C> <1|2|3>'");
                None
            },
        }
        
    }*/

    fn process_input(line : &str) -> Option<crate::network_communication::Input> {
        match line {
            cmd if cmd.starts_with(Commands::Help.to_string()) => { Self::print_help(); None }
            cmd if cmd.starts_with(Commands::Peers.to_string()) => { Some(crate::network_communication::Input::ListPeers) }
            cmd if cmd.starts_with(Commands::Turn.to_string()) => {
                parse_coords(line).map(|(x, y)| crate::network_communication::Input::Turn(x, y) )
            }
            cmd if cmd.starts_with(Commands::Start.to_string()) => { 
                cmd.strip_prefix("start ")
                .map(|index| index.parse()).unwrap().ok()
                .map(crate::network_communication::Input::InitiateGame)
            }
            cmd if cmd == "y" || cmd == "yes" => {
                Some(crate::network_communication::Input::Yes)
            }
            cmd if cmd == "n" || cmd == "no" => {Some(crate::network_communication::Input::No) }
            _ => {
                None
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
    pub fn to_string(&self) -> &'static str {
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

fn parse_coords(line: &str) -> Option<crate::network_communication::Coordinates> {
    let rest = line.strip_prefix("turn ");
    let coords : Vec<&str> = rest.unwrap().split_whitespace().collect();

    if coords.len() != 2 {
        println!("Invalid number of arguments. Expected: 2.");
        return None;
    }
    
    let x = coords[0].parse::<char>();
    let y = coords[1].parse::<usize>();
    
    match (x, y) {
        (Ok(x), Ok(y)) => convert_coords(x, y),
        (_, _) => {println!("Error while parsing arguments."); None}
    }
}

fn convert_coords(x: char, y: usize) -> Option<crate::network_communication::Coordinates> {
    let x= match x {
        'A' => Some(0),
        'B' => Some(1),
        'C' => Some(2),
        _ => return None,
    };

    if (1..=3).contains(&y)  {
        Some((x.unwrap(), y-1))
    } else {
        println!("Value is not valid, use value 1-3.");
        None
    }
}
