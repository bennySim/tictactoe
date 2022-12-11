//! # TicTacToe
//!
//! Library for simple tic tac toe game 


// TODO add counting who wins how many times 

/// Represents symbols on game playmat
#[derive(Copy,Clone,PartialEq, Debug)]
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

/// Represents 3x3 playmat
type State = [[Tile; 3]; 3];

#[derive(PartialEq, Debug, Clone)]
enum Player {
    You,
    Opponent,
    Noone,
}

impl Player {
    /// Returns player tile
    fn tile(&self) -> Tile {
        match self {
            Player::You => Tile::Circle,
            Player::Opponent => Tile::Cross,
            Player::Noone => Tile::Empty,
        }
    }
}

/// Diagonal type
pub enum Diagonal {
    Direct,
    Undirect,
    Middle,
}

pub enum CoordinateError {
    InvalidX,
    InvalidY,
}

pub enum GameError {
    InvalidValue,
    OccupiedField,
}

/// Main structure handling game logic
#[derive(Clone, Debug)]
pub struct TicTacToe {
    state: State,
    winner: Player,
}

impl TicTacToe {
    /// Creates new game
    pub fn new() -> TicTacToe {
        TicTacToe { 
            state: [[Tile::Empty; 3]; 3],
            winner: Player::Noone,
         }
    }

    /// Evaluates my turn
    pub fn make_my_turn(&mut self, x: usize, y: usize) -> Result<(), GameError> {
        self.make_turn_universal(Player::You, x, y)
    }

    /// Evaluates opponent's turn
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

    /// Returns true when I won
    pub fn am_i_winner(&self) -> bool {
        self.winner == Player::You
    }

    /// Returns true when opponent won
    pub fn is_opponent_winner(&self) -> bool {
        self.winner == Player::Opponent
    }

    /// Returns state as array of chars
    pub fn get_state(&mut self) -> [[char; 3]; 3] {
        self.state
        .map(|arr| arr
            .map(|tile| tile.to_char()))
    }

    /// Allows starting new game with same players
    /// TODO - Game should be separated from players.
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

        let mut diagonal = TicTacToe::get_diagonal(self, x, y).unwrap_or_default();
        options.append(&mut diagonal);

        options
        .iter()
        .map(|array| array
            .iter()
            .fold(true, |res, &t| (t == tile) && res))
        .any(|r| r)
    }

    fn get_diagonal(&mut self, x: usize, y:usize) -> Option<Vec<[Tile; 3]>> {
        fn get_direct_diagonal(state: [[Tile; 3]; 3])-> [Tile; 3] {
            [0, 1, 2].map(|index| state[index][index])
        }

        fn get_indirect_diagonal(state: [[Tile; 3]; 3])-> [Tile; 3] {
            [0, 1, 2].map(|index| state[index][index])
        }

        match TicTacToe::get_diagonal_type(x, y) {
            Some(Diagonal::Direct) => Some(vec![get_direct_diagonal(self.state)]),
            Some(Diagonal::Undirect) => Some(vec![get_indirect_diagonal(self.state)]),
            Some(Diagonal::Middle) => Some(vec![get_direct_diagonal(self.state), get_indirect_diagonal(self.state)]),
            None => None,
        }
    }

    fn get_diagonal_type(x: usize, y: usize) -> Option<Diagonal> {
        match (x, y) {
            (0, 0) | (2, 2) => Some(Diagonal::Direct),
            (2, 0) | (0, 2) => Some(Diagonal::Undirect),
            (1, 1) => Some(Diagonal::Middle),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::Arbitrary;

    use super::*;

    fn check_win_brute_force(state : [[Tile; 3]; 3], tile : Tile, x : usize, y : usize) -> bool{
        // col
        if state[0][y] == tile
        && state[1][y] == tile
        && state[2][y] == tile {
            return true;
        }

        // row
        if state[x][0] == tile
        && state[x][1] == tile
        && state[x][2] == tile {
            return true;
        }

        // direct diagonal
        if [(0,0), (1,1), (2,2)].contains(&(x,y)) 
            && state[0][0] == tile
            && state[1][1] == tile
            && state[2][2] == tile {
            return true;
        }

        
        // undirect diagonal
        if [(2,0), (1,1), (0,2)].contains(&(x,y))
            && state[2][0] == tile
            && state[1][1] == tile
            && state[0][2] == tile {
                return true;
        }
        false
    }

    impl Arbitrary for Tile {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            g.choose(&[Tile::Empty, Tile::Circle, Tile::Cross]).unwrap().to_owned()
        }
    }

    impl Arbitrary for TicTacToe {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut game = TicTacToe::new();
            for i in 0..3 {
                for j in 0..3 {
                    game.state[i][j] = Tile::arbitrary(g);
                }
            }
            game
        }
    }

    #[derive(Debug, Clone)]
    enum Indices {
        Zero = 0,
        One,
        Two
    }
    impl Indices {
        pub fn get_int(&self) -> usize {
            match self {
                Indices::Zero => 0,
                Indices::One => 1,
                Indices::Two => 2,
            }
        }
    }

    impl Arbitrary for Indices {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            g.choose(&[Indices::Zero, Indices::One, Indices::Two]).unwrap().to_owned()
        }
    }

    quickcheck! {
          fn check_win(game : TicTacToe, x : Indices, y : Indices) -> bool {
            assert_eq!(check_win_brute_force(game.clone().state, Tile::Circle, x.get_int(), y.get_int()) ,game.clone().check_win(Tile::Circle, x.get_int(), y.get_int()));
            true
        }
    }

}