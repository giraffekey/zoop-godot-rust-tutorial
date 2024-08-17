use godot::classes::{ISprite2D, ITileMap, Label, Sprite2D, TileMap, Timer};
use godot::global::instance_from_id;
use godot::prelude::*;
use rand::distributions::Standard;
use rand::prelude::*;
use std::collections::HashMap;

struct GoopExtension;

#[gdextension]
unsafe impl ExtensionLibrary for GoopExtension {}

const GRID_WIDTH: usize = 18;
const GRID_HEIGHT: usize = 12;
const CENTER_SIZE: usize = 4;
// X coordinate of center ranges from 7-10
const MIN_CENTER_X: usize = GRID_WIDTH / 2 - CENTER_SIZE / 2;
const MAX_CENTER_X: usize = GRID_WIDTH / 2 + CENTER_SIZE / 2 - 1;
// X coordinate of center ranges from 4-7
const MIN_CENTER_Y: usize = GRID_HEIGHT / 2 - CENTER_SIZE / 2;
const MAX_CENTER_Y: usize = GRID_HEIGHT / 2 + CENTER_SIZE / 2 - 1;

type EnemyId = usize;

#[derive(Debug, Clone, Copy, Default)]
struct Position {
    x: usize,
    y: usize,
}

impl Position {
    // Converts field position to screen coords
    fn to_vector(&self) -> Vector2 {
        Vector2::new(self.x as f32 * 16.0 + 8.0, self.y as f32 * 16.0 + 8.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    fn opposite(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Color {
    #[default]
    Red,
    Green,
    Blue,
    Purple,
}

// Allows colors to be randomly generated
impl Distribution<Color> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Color {
        match rng.gen_range(0..4) {
            0 => Color::Red,
            1 => Color::Green,
            2 => Color::Blue,
            3 => Color::Purple,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum Tile {
    #[default]
    None,
    Player,
    Enemy(EnemyId),
}

#[derive(GodotClass)]
#[class(init, base=TileMap)]
struct Field {
    rng: ThreadRng,
    grid: [[Tile; GRID_HEIGHT]; GRID_WIDTH],
    next_enemy_id: EnemyId,
    // Used to associate enemy IDs with Godot instances
    enemies: HashMap<EnemyId, i64>,
    last_direction: Option<Direction>,
    goops: u16,
    base: Base<TileMap>,
}

#[godot_api]
impl ITileMap for Field {
    fn ready(&mut self) {
        self.rng = thread_rng();

        for x in 0..GRID_WIDTH {
            for y in 0..GRID_HEIGHT {
                // True if x is within 4x4 player field
                let x_in_center = x >= MIN_CENTER_X && x <= MAX_CENTER_X;
                // True if y is within 4x4 player field
                let y_in_center = y >= MIN_CENTER_Y && y <= MAX_CENTER_Y;

                let (i, j) = match (x_in_center, y_in_center) {
                    // If both are in center, use tile is in center. Use sprite located at (0, 0).
                    (true, true) => (0, 0),
                    // If only one is in center, use tile is on edge. Use sprite located at (1, 0).
                    (true, false) | (false, true) => (1, 0),
                    // If neither are in center, use tile is in corner. Use sprite located at (2, 0).
                    (false, false) => (2, 0),
                };

                self.base_mut()
                    // first argument is layer
                    // second argument is position of cell in tilemap
                    .set_cell_ex(0, Vector2i::new(x as i32, y as i32))
                    // the ID of the tileset
                    .source_id(0)
                    // coordinates of the sprite in the tileset
                    .atlas_coords(Vector2i::new(i, j))
                    .done();
            }
        }
    }
}

#[godot_api]
impl Field {
    #[func]
    fn spawn_enemy(&mut self) {
        let mut positions = Vec::new();

        // Enemies cannot spawn in the same quadrant twice in a row
        if self.last_direction != Some(Direction::Down) {
            // Creates a list of positions at the top of the field
            // Enemy is facing the down direction
            positions.extend(
                (MIN_CENTER_X..=MAX_CENTER_X).map(|x| (Direction::Down, Position { x, y: 0 })),
            )
        }

        if self.last_direction != Some(Direction::Up) {
            positions.extend((MIN_CENTER_X..=MAX_CENTER_X).map(|x| {
                (
                    Direction::Up,
                    Position {
                        x,
                        y: GRID_HEIGHT - 1,
                    },
                )
            }))
        }

        if self.last_direction != Some(Direction::Right) {
            positions.extend(
                (MIN_CENTER_Y..=MAX_CENTER_Y).map(|y| (Direction::Right, Position { x: 0, y })),
            )
        }

        if self.last_direction != Some(Direction::Left) {
            positions.extend((MIN_CENTER_Y..=MAX_CENTER_Y).map(|y| {
                (
                    Direction::Left,
                    Position {
                        x: GRID_WIDTH - 1,
                        y,
                    },
                )
            }))
        }

        // Choose a random position at the end of one of the quadrants
        let (direction, position) = positions.choose(&mut self.rng).unwrap();
        self.last_direction = Some(*direction);

        // Move all enemies closer to the center
        match direction {
            Direction::Right => {
                for i in (0..GRID_WIDTH / 2).rev() {
                    match self.grid[i][position.y] {
                        Tile::Enemy(enemy_id) => {
                            let mut enemy: Gd<Enemy> =
                                instance_from_id(self.enemies[&enemy_id]).unwrap().cast();
                            enemy.bind_mut().move_to(Position {
                                x: i + 1,
                                y: position.y,
                            });
                            // Current position will be overwritten in a later loop
                            self.grid[i + 1][position.y] = self.grid[i][position.y];
                        }
                        _ => (),
                    }
                }
            }
            Direction::Left => {
                for i in GRID_WIDTH / 2..GRID_WIDTH {
                    match self.grid[i][position.y] {
                        Tile::Enemy(enemy_id) => {
                            let mut enemy: Gd<Enemy> =
                                instance_from_id(self.enemies[&enemy_id]).unwrap().cast();
                            enemy.bind_mut().move_to(Position {
                                x: i - 1,
                                y: position.y,
                            });
                            self.grid[i - 1][position.y] = self.grid[i][position.y];
                        }
                        _ => (),
                    }
                }
            }
            Direction::Down => {
                for i in (0..GRID_HEIGHT / 2).rev() {
                    match self.grid[position.x][i] {
                        Tile::Enemy(enemy_id) => {
                            let mut enemy: Gd<Enemy> =
                                instance_from_id(self.enemies[&enemy_id]).unwrap().cast();
                            enemy.bind_mut().move_to(Position {
                                x: position.x,
                                y: i + 1,
                            });
                            self.grid[position.x][i + 1] = self.grid[position.x][i];
                        }
                        _ => (),
                    }
                }
            }
            Direction::Up => {
                for i in GRID_HEIGHT / 2..GRID_HEIGHT {
                    match self.grid[position.x][i] {
                        Tile::Enemy(enemy_id) => {
                            let mut enemy: Gd<Enemy> =
                                instance_from_id(self.enemies[&enemy_id]).unwrap().cast();
                            enemy.bind_mut().move_to(Position {
                                x: position.x,
                                y: i - 1,
                            });
                            self.grid[position.x][i - 1] = self.grid[position.x][i];
                        }
                        _ => (),
                    }
                }
            }
        }

        // Instantiate a new enemy from the enemy scene
        let scene = load::<PackedScene>("res://enemy.tscn");
        let mut enemy: Gd<Enemy> = scene.instantiate().unwrap().cast();
        let instance_id = enemy.instance_id().to_i64();
        enemy.bind_mut().set_color(self.rng.gen());
        enemy.set_position(position.to_vector());

        let mut root = self.base().get_node_as::<Node2D>("..");
        root.add_child(enemy.clone());

        // Add the enemy to the field data
        self.grid[position.x][position.y] = Tile::Enemy(self.next_enemy_id);
        self.enemies.insert(self.next_enemy_id, instance_id);
        self.next_enemy_id += 1;

        // If any enemy as reached the center, restart the level
        if self.check_lose_condition() {
            self.base().get_tree().unwrap().reload_current_scene();
        }
    }

    fn get_enemy(&self, enemy_id: EnemyId) -> Gd<Enemy> {
        instance_from_id(self.enemies[&enemy_id]).unwrap().cast()
    }

    // Check if an enemy has reached the center
    fn check_lose_condition(&self) -> bool {
        for x in MIN_CENTER_X..=MAX_CENTER_X {
            for y in MIN_CENTER_Y..=MAX_CENTER_Y {
                match self.grid[x][y] {
                    Tile::Enemy(_) => return true,
                    _ => (),
                }
            }
        }
        false
    }

    // This function finds the closest enemy from `position` in `direction`
    fn find_enemy(&self, position: Position, direction: Direction) -> Option<(EnemyId, Position)> {
        match direction {
            Direction::Left => {
                for x in (0..position.x).rev() {
                    match self.grid[x][position.y] {
                        // Break out of loop at the first enemy found
                        Tile::Enemy(enemy_id) => {
                            return Some((enemy_id, Position { x, y: position.y }))
                        }
                        _ => (),
                    }
                }
            }
            Direction::Right => {
                for x in position.x + 1..GRID_WIDTH {
                    match self.grid[x][position.y] {
                        Tile::Enemy(enemy_id) => {
                            return Some((enemy_id, Position { x, y: position.y }))
                        }
                        _ => (),
                    }
                }
            }
            Direction::Up => {
                for y in (0..position.y).rev() {
                    match self.grid[position.x][y] {
                        Tile::Enemy(enemy_id) => {
                            return Some((enemy_id, Position { x: position.x, y }))
                        }
                        _ => (),
                    }
                }
            }
            Direction::Down => {
                for y in position.y + 1..GRID_HEIGHT {
                    match self.grid[position.x][y] {
                        Tile::Enemy(enemy_id) => {
                            return Some((enemy_id, Position { x: position.x, y }))
                        }
                        _ => (),
                    }
                }
            }
        }
        None
    }

    fn remove_enemy(&mut self, enemy_id: EnemyId, position: Position) {
        let mut enemy = self.get_enemy(enemy_id);
        enemy.queue_free();
        self.enemies.remove(&enemy_id);
        self.grid[position.x][position.y] = Tile::None;
    }

    fn add_goops(&mut self, goops: u16) {
        self.goops += goops;
        // Enemies spawn 10% faster for every 20 enemies killed
        let wait_time = 0.9_f64.powf((self.goops / 20) as f64);

        let mut timer = self.base().get_node_as::<Timer>("Timer");
        timer.set_wait_time(wait_time);
    }
}

#[derive(GodotClass)]
#[class(init, base=Label)]
struct Score {
    points: u16,
    base: Base<Label>,
}

impl Score {
    fn add_points(&mut self, goops: u16) {
        // Killing multiple enemies in one move gives bonus points
        for i in 1..=goops {
            self.points += 100 * i;
        }

        let text = self.points.to_string().into();
        self.base_mut().set_text(text);
    }
}

#[derive(GodotClass)]
#[class(init, base=Sprite2D)]
struct Player {
    position: Position,
    direction: Direction,
    color: Color,
    is_moving: bool,
    is_shooting: bool,
    base: Base<Sprite2D>,
}

#[godot_api]
impl ISprite2D for Player {
    fn ready(&mut self) {
        let mut field = self.base().get_node_as::<Field>("../Field");
        let mut field = field.bind_mut();

        self.set_color(field.rng.gen());
        self.set_direction(Direction::Up);

        // Set the player's position at a random position in the center
        let x = field.rng.gen_range(MIN_CENTER_X..=MAX_CENTER_X);
        let y = field.rng.gen_range(MIN_CENTER_Y..=MAX_CENTER_Y);
        self.set_position(Position { x, y }, &mut field);
    }

    fn process(&mut self, _dt: f64) {
        if !self.is_shooting {
            let input = Input::singleton();

            // Move in the direction of button press
            if !self.is_moving {
                if input.is_action_just_pressed("left".into()) {
                    self.set_direction(Direction::Left);
                    self.move_to(-1, 0);
                } else if input.is_action_just_pressed("right".into()) {
                    self.set_direction(Direction::Right);
                    self.move_to(1, 0);
                } else if input.is_action_just_pressed("up".into()) {
                    self.set_direction(Direction::Up);
                    self.move_to(0, -1);
                } else if input.is_action_just_pressed("down".into()) {
                    self.set_direction(Direction::Down);
                    self.move_to(0, 1);
                }
            }

            if input.is_action_just_pressed("shoot".into()) {
                let mut field = self.base().get_node_as::<Field>("../Field");
                let mut field = field.bind_mut();

                let mut position = self.position;
                let mut goops = 0;

                // Kill every enemy of the same color until one can no longer be found
                while let Some((enemy_id, enemy_position)) =
                    field.find_enemy(position, self.direction)
                {
                    let mut enemy = field.get_enemy(enemy_id);
                    let mut enemy = enemy.bind_mut();

                    // Updating the position reduces the required computation for `find_enemy`
                    position = enemy_position;

                    if self.color == enemy.color {
                        field.remove_enemy(enemy_id, enemy_position);
                        goops += 1;
                    } else {
                        // If the color does not match, swap the player and enemy color, then break out of loop
                        let color = self.color;
                        self.set_color(enemy.color);
                        enemy.set_color(color);
                        break;
                    }
                }

                // This increases the difficulty for each kill
                field.add_goops(goops);

                // Increase score based on number of killed enemies
                if goops > 0 {
                    let mut score = self.base().get_node_as::<Score>("../Score");
                    let mut score = score.bind_mut();
                    score.add_points(goops);
                }

                self.shoot(position);
                self.is_shooting = true;
            }
        }
    }
}

#[godot_api]
impl Player {
    #[func]
    fn end_movement(&mut self) {
        self.is_moving = false;
    }

    // Return to original position after shooting
    #[func]
    fn return_to_position(&mut self) {
        let mut tween = self.base_mut().create_tween().unwrap();
        tween.tween_property(
            self.base().clone(),
            "position".into(),
            Variant::from(self.position.to_vector()),
            0.15,
        );
        tween.tween_callback(Callable::from_object_method(&self.base(), "end_shoot"));

        self.set_direction(self.direction.opposite());
    }

    #[func]
    fn end_shoot(&mut self) {
        self.set_direction(self.direction.opposite());
        self.is_shooting = false;
    }

    fn set_position(&mut self, position: Position, field: &mut Field) {
        // Update player's position in grid
        field.grid[self.position.x][self.position.y] = Tile::None;
        field.grid[position.x][position.y] = Tile::Player;

        self.position = position;
        self.base_mut().set_position(position.to_vector());
    }

    fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;

        // Change the sprite's rotation based on new direction
        match direction {
            Direction::Left => self.base_mut().set_rotation_degrees(270.0),
            Direction::Right => self.base_mut().set_rotation_degrees(90.0),
            Direction::Up => self.base_mut().set_rotation_degrees(0.0),
            Direction::Down => self.base_mut().set_rotation_degrees(180.0),
        }
    }

    fn set_color(&mut self, color: Color) {
        self.color = color;

        // Change the sprite's region based on new color
        let position = match color {
            Color::Red => Vector2::new(0.0, 16.0),
            Color::Green => Vector2::new(16.0, 16.0),
            Color::Blue => Vector2::new(32.0, 16.0),
            Color::Purple => Vector2::new(48.0, 16.0),
        };
        self.base_mut()
            .set_region_rect(Rect2::new(position, Vector2::new(16.0, 16.0)));
    }

    fn move_to(&mut self, dx: isize, dy: isize) {
        let mut next_x = (self.position.x as isize + dx) as usize;
        let mut next_y = (self.position.y as isize + dy) as usize;

        // Prevent player from going out of x bounds
        if next_x < MIN_CENTER_X {
            next_x = MIN_CENTER_X;
        } else if next_x > MAX_CENTER_X {
            next_x = MAX_CENTER_X;
        }

        // Prevent player from going out of y bounds
        if next_y < MIN_CENTER_Y {
            next_y = MIN_CENTER_Y;
        } else if next_y > MAX_CENTER_Y {
            next_y = MAX_CENTER_Y;
        }

        self.position.x = next_x;
        self.position.y = next_y;

        // Tween to the next screen position
        let mut tween = self.base_mut().create_tween().unwrap();
        tween.tween_property(
            self.base().clone(),
            "position".into(),
            Variant::from(self.position.to_vector()),
            0.1,
        );
        tween.tween_callback(Callable::from_object_method(&self.base(), "end_movement"));

        self.is_moving = true;
    }

    // Move to the position that was shot at
    fn shoot(&mut self, position: Position) {
        let mut tween = self.base_mut().create_tween().unwrap();
        tween.tween_property(
            self.base().clone(),
            "position".into(),
            Variant::from(position.to_vector()),
            0.15,
        );
        tween.tween_callback(Callable::from_object_method(
            &self.base(),
            "return_to_position",
        ));
    }
}

#[derive(GodotClass)]
#[class(init, base=Sprite2D)]
struct Enemy {
    color: Color,
    base: Base<Sprite2D>,
}

impl Enemy {
    fn set_color(&mut self, color: Color) {
        self.color = color;

        // Change the sprite's region based on new color
        let position = match color {
            Color::Red => Vector2::new(0.0, 32.0),
            Color::Green => Vector2::new(16.0, 32.0),
            Color::Blue => Vector2::new(32.0, 32.0),
            Color::Purple => Vector2::new(48.0, 32.0),
        };
        self.base_mut()
            .set_region_rect(Rect2::new(position, Vector2::new(16.0, 16.0)));
    }

    fn move_to(&mut self, position: Position) {
        // Tween to the next screen position
        let mut tween = self.base_mut().create_tween().unwrap();
        tween.tween_property(
            self.base().clone(),
            "position".into(),
            Variant::from(position.to_vector()),
            0.1,
        );
        tween.tween_callback(Callable::from_object_method(&self.base(), "end_movement"));
    }
}
