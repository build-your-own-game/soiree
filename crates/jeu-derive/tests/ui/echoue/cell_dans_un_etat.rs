// `Cell` ne vit pas dans un état : mutabilité intérieure ou ordre variable (§ 2.7, § 8.2).
use core::cell::Cell;

#[jeu::etat]
pub struct Etat {
    pub champ: Cell<u8>,
}

fn main() {}
