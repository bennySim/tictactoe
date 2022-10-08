#[derive(Copy,Clone,PartialEq)]
enum Tile {
    Cross,
    Circle,
    Empty,
}

impl Tile {
    fn to_char(&self) -> char {
        match self {
            Tile::Cross => 'X',
            Tile::Circle => 'O',
            Tile::Empty => ' ',
        }
    }
}

type State = [[Tile; 3]; 3];

#[derive(PartialEq)]
enum Player {
    You,
    Opponent,
    Noone,
}

impl Player {
    fn tile(&self) -> Tile {
        match self {
            Player::You => Tile::Circle,
            Player::Opponent => Tile::Cross,
            Player::Noone => Tile::Empty,
        }
    }
}

pub enum Diagonal {
    Direct,
    Undirect,
}

pub struct TicTacToe {
    state: State,
    winner: Player,
}

pub enum CoordinateError {
    InvalidX,
    InvalidY,
}

pub enum GameError {
    InvalidValue,
    OccupiedField,
}

impl TicTacToe {
    pub fn new() -> TicTacToe {
        TicTacToe { 
            state: [[Tile::Empty; 3]; 3],
            winner: Player::Noone,
         }
    }

    pub fn make_my_turn(&mut self, x: usize, y: usize) -> Result<(), GameError> {
        self.make_turn_universal(Player::You, x, y)
    }

    pub fn make_opponent_turn(&mut self, x: usize, y: usize) -> Result<(), GameError> {
        self.make_turn_universal(Player::Opponent, x, y)
    }

    fn make_turn_universal(&mut self, player : Player, x: usize, y: usize) -> Result<(), GameError> {

        if !(0..=2).contains(&x) || !(0..=2).contains(&y) {
            return Err(GameError::InvalidValue);
        }

        if self.state[x][y] != Tile::Empty {
            return Err(GameError::OccupiedField);
        }

        let is_winning_turn = self.make_turn(player.tile(), x, y);
        if is_winning_turn {
            self.winner = player;
        }

        Ok(())
    }

    pub fn am_i_winner(&self) -> bool {
        self.winner == Player::You
    }

    pub fn is_opponent_winner(&self) -> bool {
        self.winner == Player::Opponent
    }

    pub fn get_state(&mut self) -> [[char; 3]; 3] {
        self.state
        .map(|arr| arr
            .map(|tile| tile.to_char()))
    }

    pub fn reset(&mut self) {
        self.state = [[Tile::Empty; 3]; 3];
        self.winner = Player::Noone;
    }

    fn make_turn(&mut self, tile: Tile, x: usize, y: usize) -> bool {
        self.state[x][y] = tile;
        self.check_win(tile, x, y)
    }

    fn check_win(&mut self, tile: Tile, x: usize, y: usize) -> bool {
        let col: [Tile; 3] = [0, 1 ,2].map(|index| self.state[index][y]);
        let row: [Tile; 3] = self.state[x];
        let mut options = vec![col, row];

        if let Some(diagonal) = TicTacToe::get_diagonal(self, x, y) {
            options.push(diagonal);
        }

        options
        .iter()
        .map(|array| array
            .iter()
            .fold(true, |res, &t| (t == tile) && res))
        .any(|r| r)
    }

    fn get_diagonal(&mut self, x: usize, y:usize) -> Option<[Tile; 3]> {
        match TicTacToe::is_corner(x, y) {
            Some(Diagonal::Direct) => Some([0, 1, 2].map(|index| self.state[index][index])),
            Some(Diagonal::Undirect) => Some([(0, 2) ,(1, 1), (2, 0)].map(|(row, col)| self.state[row][col])),
            None => None,
        }
    }

    fn is_corner(x: usize, y: usize) -> Option<Diagonal> {
        match (x, y) {
            (0, 0) | (2, 2) => Some(Diagonal::Direct),
            (2, 0) | (0, 2) => Some(Diagonal::Undirect),
            _ => None,
        }
    }
}