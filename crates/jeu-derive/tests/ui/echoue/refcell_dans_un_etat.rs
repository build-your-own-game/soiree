// `RefCell` ne vit pas dans un état : mutabilité intérieure ou ordre variable (§ 2.7, § 8.2).
use core::cell::RefCell;

#[jeu::etat]
pub struct Etat {
    pub champ: RefCell<u8>,
}

fn main() {}
