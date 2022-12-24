use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub trait PrintToOutput {
    fn print_table(grid : [[char; 3]; 3]);

    fn print_help();

    fn print_string(text : &str);

    fn process_coords(line: &str) -> Option<Coordinates>;
}

pub struct PrintToOutputStdio {}

impl PrintToOutput for PrintToOutputStdio {
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
    
    
}

type Coordinates = (usize, usize);

enum CoordinatesError {
    InvalidFormat,
    InvalidValue,
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

fn convert_coords(x: char, y: usize) -> Option<Coordinates> {
    let x= match x {
        'A' => Some(0),
        'B' => Some(1),
        'C' => Some(2),
        _ => return None,
    };

    if (1..=3).contains(&y)  {
        Some((x.unwrap(), y-1))
    } else {
        None
    }
}
